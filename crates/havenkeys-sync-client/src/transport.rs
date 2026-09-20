//! How bytes reach the server.
//!
//! The protocol code does not know about `reqwest`: it hands a `HttpRequest`
//! to a `Transport` and gets bytes back. That keeps a `fetch`-based
//! implementation possible for a future extension build (design §11), and it
//! lets the hostile-server suite feed the client crafted answers with no
//! socket in the way.

use crate::error::{Result, SyncError};
use std::future::Future;
use zeroize::Zeroizing;

/// The largest answer the protocol can legitimately produce: a full pull page
/// of blobs, base64-expanded, plus slack. Anything beyond it is refused while
/// still being read, so a server cannot make the client allocate at will.
pub const MAX_RESPONSE_BYTES: usize = 17 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

pub struct HttpRequest {
    pub method: Method,
    /// Path and query, always beginning with `/v1/`.
    pub path: String,
    pub token: Option<Zeroizing<String>>,
    pub body: Option<Vec<u8>>,
}

impl std::fmt::Debug for HttpRequest {
    /// The body carries ciphertext and the token is a secret; neither is
    /// printable.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("authenticated", &self.token.is_some())
            .field("body_bytes", &self.body.as_ref().map(Vec::len))
            .finish()
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait Transport {
    fn send(&self, request: HttpRequest) -> impl Future<Output = Result<HttpResponse>> + Send;
}

/// The real transport.
#[derive(Debug)]
pub struct HttpTransport {
    base: url::Url,
    client: reqwest::Client,
}

impl HttpTransport {
    /// Refuses any base URL a bearer token must not travel to: plain HTTP
    /// anywhere but localhost, and anything that is not http(s).
    pub fn new(base_url: &str) -> Result<Self> {
        let base = url::Url::parse(base_url.trim_end_matches('/'))
            .map_err(|_| SyncError::InvalidServerUrl)?;
        let host = base.host_str().unwrap_or_default().to_string();
        let local = host == "localhost" || host == "127.0.0.1" || host == "[::1]";
        match base.scheme() {
            "https" => {}
            "http" if local => {}
            _ => return Err(SyncError::InvalidServerUrl),
        }
        let client = reqwest::Client::builder()
            // Following a redirect would hand the token to a host the user
            // never chose.
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .user_agent("havenkeys")
            .build()
            .map_err(|_| SyncError::Unavailable)?;
        Ok(Self { base, client })
    }

    pub fn base_url(&self) -> &str {
        self.base.as_str()
    }
}

impl Transport for HttpTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        let url = self
            .base
            .join(&request.path)
            .map_err(|_| SyncError::InvalidServerUrl)?;
        // `join` on an absolute path could otherwise move the request to
        // another host if `path` were ever attacker-influenced.
        if url.host_str() != self.base.host_str() || url.scheme() != self.base.scheme() {
            return Err(SyncError::InvalidServerUrl);
        }

        let mut builder = self
            .client
            .request(
                reqwest::Method::from_bytes(request.method.as_str().as_bytes())
                    .map_err(|_| SyncError::Protocol("method"))?,
                url,
            )
            .header("accept", "application/json");
        if let Some(token) = &request.token {
            builder = builder.bearer_auth(token.as_str());
        }
        if let Some(body) = request.body {
            builder = builder
                .header("content-type", "application/json")
                .body(body);
        }

        let mut response = builder.send().await.map_err(|_| SyncError::Unavailable)?;
        let status = response.status().as_u16();

        // Read in chunks and stop at the cap. `Content-Length` is the
        // server's claim about itself, so it decides nothing here.
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| SyncError::Unavailable)? {
            if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(SyncError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse { status, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_or_localhost_is_accepted() {
        assert!(HttpTransport::new("https://vault.example.com").is_ok());
        assert!(HttpTransport::new("http://localhost:8080").is_ok());
        assert!(HttpTransport::new("http://127.0.0.1:8080").is_ok());
        assert_eq!(
            HttpTransport::new("http://vault.example.com").unwrap_err(),
            SyncError::InvalidServerUrl
        );
        assert_eq!(
            HttpTransport::new("ftp://vault.example.com").unwrap_err(),
            SyncError::InvalidServerUrl
        );
        assert_eq!(
            HttpTransport::new("not a url").unwrap_err(),
            SyncError::InvalidServerUrl
        );
    }

    #[test]
    fn a_request_debug_shows_no_token_and_no_body() {
        let request = HttpRequest {
            method: Method::Post,
            path: "/v1/items".into(),
            token: Some(Zeroizing::new("secret-token".into())),
            body: Some(b"{\"changes\":[]}".to_vec()),
        };
        let shown = format!("{request:?}");
        assert!(!shown.contains("secret-token"));
        assert!(!shown.contains("changes"));
        assert!(shown.contains("authenticated: true"));
    }
}

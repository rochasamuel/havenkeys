//! The one network request M1 makes besides the user's server (spec §6.3):
//! `https://<site>/.well-known/assetlinks.json`, for sites already in the
//! vault, when the Digital Asset Links setting is on.

use havenkeys_core::asset_links::{parse, AppStatement, MAX_ASSET_LINKS_BYTES};
use std::time::Duration;

pub(crate) const FETCH_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) fn client() -> Option<reqwest::Client> {
    havenkeys_sync_client::http_client_builder()
        .timeout(FETCH_TIMEOUT)
        .connect_timeout(FETCH_TIMEOUT)
        .build()
        .ok()
}

/// `None` for anything that is not a plain DNS name.
pub(crate) fn url_for(host: &str) -> Option<String> {
    let url = url::Url::parse(&format!("https://{host}/.well-known/assetlinks.json")).ok()?;
    match url.host() {
        Some(url::Host::Domain(d)) if d == host && d != "localhost" && d.contains('.') => {
            Some(url.into())
        }
        _ => None,
    }
}

/// `None` on any failure: unreachable, non-200 (redirects included — the
/// client never follows one), too large, not a valid file.
pub(crate) async fn fetch_from(client: &reqwest::Client, url: &str) -> Option<Vec<AppStatement>> {
    let mut response = client.get(url).send().await.ok()?;
    if response.status() != reqwest::StatusCode::OK {
        return None;
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if body.len() + chunk.len() > MAX_ASSET_LINKS_BYTES {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    parse(&body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;

    async fn serve(app: axum::Router) -> String {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn a_file_is_fetched_and_parsed() {
        let body = r#"[{"relation":["delegate_permission/common.get_login_creds"],"target":{"namespace":"android_app","package_name":"com.x.y","sha256_cert_fingerprints":["AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99"]}}]"#;
        let base = serve(axum::Router::new().route(
            "/.well-known/assetlinks.json",
            get(move || async move { body }),
        ))
        .await;
        let s = fetch_from(
            &client().unwrap(),
            &format!("{base}/.well-known/assetlinks.json"),
        )
        .await
        .unwrap();
        assert_eq!(s.len(), 1);
    }

    #[tokio::test]
    async fn a_redirect_is_not_followed() {
        let base = serve(axum::Router::new().route(
            "/.well-known/assetlinks.json",
            get(|| async { axum::response::Redirect::temporary("https://evil.example/") }),
        ))
        .await;
        assert!(fetch_from(
            &client().unwrap(),
            &format!("{base}/.well-known/assetlinks.json")
        )
        .await
        .is_none());
    }

    #[tokio::test]
    async fn an_oversized_answer_is_a_failure() {
        let big = "[".to_string()
            + &" ".repeat(havenkeys_core::asset_links::MAX_ASSET_LINKS_BYTES + 10)
            + "]";
        let base = serve(axum::Router::new().route(
            "/.well-known/assetlinks.json",
            get(move || {
                let b = big.clone();
                async move { b }
            }),
        ))
        .await;
        assert!(fetch_from(
            &client().unwrap(),
            &format!("{base}/.well-known/assetlinks.json")
        )
        .await
        .is_none());
    }

    #[test]
    fn only_plain_domain_hosts_are_asked() {
        assert!(url_for("github.com").is_some());
        for bad in [
            "192.168.1.1",
            "localhost",
            "github.com/x",
            "a@b.com",
            "",
            "[::1]",
        ] {
            assert!(url_for(bad).is_none(), "{bad}");
        }
    }
}

//! "Change password" on a login (spec 2026-11-14-change-password-url-design).
//!
//! The renderer names the login only. Rust picks what to open, in order:
//!
//! 1. the curated change-password page for one of its websites
//!    (`havenkeys_core::change_password`, compiled in, no network);
//! 2. otherwise `https://<host>/.well-known/change-password`, if a probe of
//!    the site says it is real: the "should not exist" URL must not answer
//!    2xx (a site that answers 2xx to everything gives no signal), then the
//!    well-known URL must end in a 2xx;
//! 3. otherwise the login's first website.
//!
//! The probe is the only request here, and it runs only on the user's click.
//! It tells the site what opening it in the browser would, and less: https
//! only, no cookies (reqwest is built without a cookie store), no
//! credentials, no Referer, no proxy, a fixed `havenkeys` User-Agent, short
//! timeouts, at most `MAX_REDIRECTS` redirects and each one checked by
//! `probe_may_request`. Names resolve only to public addresses
//! (`is_public_address`), so a site cannot point the probe at the user's
//! router or loopback, even by DNS rebinding. Neither response body is read;
//! only the status code is used, and what opens is the well-known URL
//! itself (the browser follows the site's redirect with the user's own
//! session), never an address taken from a response.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::change_password::{self as cp, ChangePasswordPlan};
use serde::Serialize;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use url::Url;
use uuid::Uuid;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(4);
/// The whole probe, both requests and every redirect.
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// Which of the three the command opened; the UI says so when it is the
/// plain website.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangePasswordSource {
    Curated,
    WellKnown,
    Website,
}

#[tauri::command]
pub async fn open_change_password(app: AppHandle, id: Uuid) -> CmdResult<ChangePasswordSource> {
    let plan = {
        let state = app.state::<AppState>();
        state.touch();
        let vault = state.vault()?;
        let item = vault.get_item(&id)?;
        cp::plan(&item).ok_or(havenkeys_core::Error::NotFound)?
    };
    let (target, source) = resolve(plan, |origin| async move {
        let client = client(Rules::PUBLIC, REQUEST_TIMEOUT)?;
        probe(&client, &origin, PROBE_TIMEOUT).await
    })
    .await;
    tauri_plugin_opener::open_url(target, None::<&str>).map_err(|_| CmdError::open_website())?;
    Ok(source)
}

/// What to open for `plan`. `probe` runs only when there is no curated page
/// and the login has a website that may be probed.
async fn resolve<F, Fut>(plan: ChangePasswordPlan, probe: F) -> (String, ChangePasswordSource)
where
    F: FnOnce(Url) -> Fut,
    Fut: Future<Output = Option<Url>>,
{
    if let Some(page) = plan.curated {
        return (page, ChangePasswordSource::Curated);
    }
    if let Some(origin) = plan.probe_origin {
        if let Some(found) = probe(origin).await {
            return (found.into(), ChangePasswordSource::WellKnown);
        }
    }
    (plan.website, ChangePasswordSource::Website)
}

/// What the client may reach. Only tests loosen it, to talk to a local
/// http server.
#[derive(Clone, Copy)]
struct Rules {
    public_only: bool,
}

impl Rules {
    const PUBLIC: Rules = Rules { public_only: true };
}

/// A client and the rules it was built with.
struct Prober {
    http: reqwest::Client,
    rules: Rules,
}

fn client(rules: Rules, timeout: Duration) -> Option<Prober> {
    let policy = reqwest::redirect::Policy::custom(move |attempt| {
        // `previous` holds every URL already requested, the first included.
        if attempt.previous().len() > cp::MAX_REDIRECTS
            || (rules.public_only && !cp::probe_may_request(attempt.url()))
        {
            // The 3xx itself comes back, which is not a 2xx: no signal.
            attempt.stop()
        } else {
            attempt.follow()
        }
    });
    let builder = reqwest::Client::builder()
        .redirect(policy)
        .referer(false)
        .no_proxy()
        .user_agent("havenkeys")
        .connect_timeout(CONNECT_TIMEOUT.min(timeout))
        .timeout(timeout)
        .https_only(rules.public_only);
    let builder = if rules.public_only {
        builder.dns_resolver(Arc::new(PublicResolver))
    } else {
        builder
    };
    Some(Prober {
        http: builder.build().ok()?,
        rules,
    })
}

/// The system resolver, keeping only public addresses.
struct PublicResolver;

impl reqwest::dns::Resolve for PublicResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| cp::is_public_address(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err("no public address".into());
            }
            let addrs: reqwest::dns::Addrs = Box::new(addrs.into_iter());
            Ok(addrs)
        })
    }
}

/// `origin`'s well-known change-password URL, if the site honours it.
async fn probe(prober: &Prober, origin: &Url, limit: Duration) -> Option<Url> {
    // The first request is checked here, every redirect by the policy; an
    // IP literal never reaches the resolver, so this check is what keeps it
    // out.
    if prober.rules.public_only && !cp::probe_may_request(origin) {
        return None;
    }
    tokio::time::timeout(limit, probe_inner(&prober.http, origin))
        .await
        .ok()
        .flatten()
}

async fn probe_inner(client: &reqwest::Client, origin: &Url) -> Option<Url> {
    let missing = origin.join(cp::NOT_EXIST_PATH).ok()?;
    let well_known = origin.join(cp::WELL_KNOWN_PATH).ok()?;
    // An unreachable site, or one that answers 2xx for a page that cannot
    // exist, says nothing about its well-known URL.
    let answer = client.get(missing).send().await.ok()?;
    if answer.status().is_success() {
        return None;
    }
    drop(answer);
    let answer = client.get(well_known.clone()).send().await.ok()?;
    answer.status().is_success().then_some(well_known)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::Redirect;
    use axum::routing::get;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    const LOCAL: Rules = Rules { public_only: false };
    const SHORT: Duration = Duration::from_millis(400);

    async fn serve(app: axum::Router) -> Url {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        Url::parse(&format!("http://{addr}/")).unwrap()
    }

    fn local() -> Prober {
        client(LOCAL, SHORT).unwrap()
    }

    async fn probe_local(origin: &Url) -> Option<Url> {
        probe(&local(), origin, Duration::from_secs(3)).await
    }

    #[tokio::test]
    async fn a_redirecting_well_known_url_is_found() {
        let origin = serve(
            axum::Router::new()
                .route(
                    cp::WELL_KNOWN_PATH,
                    get(|| async { Redirect::temporary("/account/password") }),
                )
                .route("/account/password", get(|| async { "change it here" })),
        )
        .await;
        let found = probe_local(&origin).await.unwrap();
        assert_eq!(
            found.path(),
            cp::WELL_KNOWN_PATH,
            "opens the well-known URL, not the target"
        );
    }

    #[tokio::test]
    async fn a_site_answering_200_to_everything_gives_no_signal() {
        let origin = serve(axum::Router::new().fallback(|| async { "hello" })).await;
        assert_eq!(probe_local(&origin).await, None);
    }

    #[tokio::test]
    async fn a_soft_404_by_redirect_gives_no_signal() {
        // Everything unknown redirects to a 200 home page.
        let origin = serve(
            axum::Router::new()
                .route("/", get(|| async { "home" }))
                .fallback(|| async { Redirect::temporary("/") }),
        )
        .await;
        assert_eq!(probe_local(&origin).await, None);
    }

    #[tokio::test]
    async fn a_missing_well_known_url_is_not_found() {
        let origin = serve(axum::Router::new().route("/", get(|| async { "home" }))).await;
        assert_eq!(probe_local(&origin).await, None);
        let origin = serve(axum::Router::new().route(
            cp::WELL_KNOWN_PATH,
            get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
        ))
        .await;
        assert_eq!(probe_local(&origin).await, None);
    }

    fn chain(hops: usize) -> axum::Router {
        // /.well-known/change-password -> /r/1 -> ... -> /r/{hops-1} -> /done
        let mut app = axum::Router::new()
            .route("/done", get(|| async { "ok" }))
            .route(
                cp::WELL_KNOWN_PATH,
                get(move || async move {
                    Redirect::temporary(if hops == 1 { "/done" } else { "/r/1" })
                }),
            );
        for i in 1..hops {
            let next = if i + 1 == hops {
                "/done".to_owned()
            } else {
                format!("/r/{}", i + 1)
            };
            app = app.route(
                &format!("/r/{i}"),
                get(move || {
                    let next = next.clone();
                    async move { Redirect::temporary(&next) }
                }),
            );
        }
        app
    }

    #[tokio::test]
    async fn redirects_are_bounded() {
        let ok = serve(chain(cp::MAX_REDIRECTS)).await;
        assert!(
            probe_local(&ok).await.is_some(),
            "{} redirects are followed",
            cp::MAX_REDIRECTS
        );
        let too_many = serve(chain(cp::MAX_REDIRECTS + 1)).await;
        assert_eq!(probe_local(&too_many).await, None);
    }

    #[tokio::test]
    async fn a_slow_site_times_out() {
        let origin = serve(axum::Router::new().route(
            cp::WELL_KNOWN_PATH,
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "late"
            }),
        ))
        .await;
        let started = std::time::Instant::now();
        assert_eq!(probe_local(&origin).await, None);
        assert!(started.elapsed() < Duration::from_secs(3));

        // The overall limit holds even when each request is quick to start.
        let started = std::time::Instant::now();
        let slow = client(LOCAL, Duration::from_secs(10)).unwrap();
        assert_eq!(probe(&slow, &origin, SHORT).await, None);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[tokio::test]
    async fn an_unreachable_site_is_not_found() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        let origin = Url::parse(&format!("http://{addr}/")).unwrap();
        assert_eq!(probe_local(&origin).await, None);
    }

    #[tokio::test]
    async fn no_cookie_credentials_or_referer_are_sent() {
        let seen: Arc<Mutex<Vec<HeaderMap>>> = Arc::default();
        let record = |seen: Arc<Mutex<Vec<HeaderMap>>>| {
            move |headers: HeaderMap| {
                seen.lock().unwrap().push(headers);
                async { Redirect::temporary("/settings") }
            }
        };
        let s2 = seen.clone();
        let origin = serve(
            axum::Router::new()
                .route(cp::WELL_KNOWN_PATH, get(record(seen.clone())))
                .route(
                    "/settings",
                    get(move |headers: HeaderMap| {
                        s2.lock().unwrap().push(headers);
                        async { ([("set-cookie", "session=abc")], "ok") }
                    }),
                ),
        )
        .await;
        let http = local();
        assert!(probe(&http, &origin, Duration::from_secs(3))
            .await
            .is_some());
        // A second probe with the same client: a cookie store would now send it.
        assert!(probe(&http, &origin, Duration::from_secs(3))
            .await
            .is_some());
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 4);
        for h in seen.iter() {
            for name in ["cookie", "authorization", "referer"] {
                assert!(h.get(name).is_none(), "{name} was sent");
            }
            assert_eq!(h.get("user-agent").unwrap(), "havenkeys");
        }
    }

    /// A listener that only counts the connections made to it.
    async fn counting_listener() -> (u16, Arc<AtomicUsize>) {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let accepted = Arc::new(AtomicUsize::new(0));
        let a = accepted.clone();
        tokio::spawn(async move {
            while let Ok((socket, _)) = l.accept().await {
                a.fetch_add(1, Ordering::SeqCst);
                drop(socket);
            }
        });
        (port, accepted)
    }

    #[tokio::test]
    async fn the_public_client_never_connects_to_local_or_plain_http_hosts() {
        let (port, accepted) = counting_listener().await;
        let public = client(Rules::PUBLIC, SHORT).unwrap();
        let targets = [
            format!("http://127.0.0.1:{port}/"),
            format!("https://127.0.0.1:{port}/"),
            format!("https://localhost:{port}/"),
            format!("http://localhost:{port}/"),
            format!("https://[::1]:{port}/"),
        ];
        for target in &targets {
            let target = Url::parse(target).unwrap();
            assert_eq!(probe(&public, &target, Duration::from_secs(3)).await, None);
        }
        // Past the probe's own check, the client itself still refuses plain
        // http and names that resolve to a local address. (An https IP
        // literal skips the resolver: the check in `probe` above, and the
        // redirect policy, are what keep it out.)
        for target in [
            format!("http://127.0.0.1:{port}/"),
            format!("http://localhost:{port}/"),
            format!("https://localhost:{port}/"),
        ] {
            assert!(public.http.get(&target).send().await.is_err(), "{target}");
        }
        assert_eq!(
            accepted.load(Ordering::SeqCst),
            0,
            "a local port was reached"
        );
    }

    #[tokio::test]
    async fn the_public_resolver_drops_loopback() {
        use reqwest::dns::Resolve;
        let name: reqwest::dns::Name = "localhost".parse().unwrap();
        assert!(PublicResolver.resolve(name).await.is_err());
    }

    fn plan(curated: Option<&str>, probe_origin: Option<&str>) -> ChangePasswordPlan {
        ChangePasswordPlan {
            curated: curated.map(str::to_owned),
            probe_origin: probe_origin.map(|o| Url::parse(o).unwrap()),
            website: "https://example.com/login".to_owned(),
        }
    }

    fn never(_: Url) -> std::future::Ready<Option<Url>> {
        panic!("probed when it should not have been")
    }

    #[tokio::test]
    async fn the_curated_page_wins_without_any_request() {
        let p = plan(Some("https://example.com/pw"), Some("https://example.com/"));
        let got = resolve(p, never).await;
        assert_eq!(
            got,
            (
                "https://example.com/pw".to_owned(),
                ChangePasswordSource::Curated
            )
        );
    }

    #[tokio::test]
    async fn the_probe_then_the_website() {
        let p = plan(None, Some("https://example.com/"));
        let got = resolve(p, |o| async move { o.join(cp::WELL_KNOWN_PATH).ok() }).await;
        assert_eq!(
            got,
            (
                "https://example.com/.well-known/change-password".to_owned(),
                ChangePasswordSource::WellKnown
            )
        );

        let p = plan(None, Some("https://example.com/"));
        let got = resolve(p, |_| async { None }).await;
        assert_eq!(
            got,
            (
                "https://example.com/login".to_owned(),
                ChangePasswordSource::Website
            )
        );

        let p = plan(None, None);
        let got = resolve(p, never).await;
        assert_eq!(got.1, ChangePasswordSource::Website);
    }
}

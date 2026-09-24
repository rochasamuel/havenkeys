//! Bridge security tests: the attacks in docs/threat-model.md §5, driven
//! through the real request parser and, where it matters, a real socket.

use havenkeys_bridge::Bridge;
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, Settings, UrlRule};
use havenkeys_core::store::{AccountRecord, Store};
use havenkeys_core::vault::{prepare_new_account_vault, VaultService};
use havenkeys_core::SecretString;
use havenkeys_protocol::endpoint::Endpoint;
use havenkeys_protocol::frame::{read_frame, write_frame};
use havenkeys_protocol::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";
const NOW: i64 = 1_700_000_000_000;

struct Fixture {
    vault: Arc<Mutex<VaultService>>,
    bridge: Bridge,
    locks: Arc<AtomicUsize>,
    changes: Arc<AtomicUsize>,
    github: Uuid,
    bank: Uuid,
    note: Uuid,
}

fn account() -> AccountRef {
    AccountRef::new(
        Uuid::from_u128(0x5eed),
        NormalizedEmail::parse("user@example.com").unwrap(),
    )
}

fn account_record() -> AccountRecord {
    AccountRecord {
        account_id: account().id,
        email: "user@example.com".into(),
        server_url: "https://vault.example.com".into(),
        server_cursor: 0,
        max_header_rev: 0,
        last_synced_at: None,
    }
}

/// An activated, unlocked account-bound vault (every vault is account-bound
/// now; see `havenkeys-core/tests/common/mod.rs::activated_vault`).
fn new_account_vault(kdf: KdfParams) -> VaultService {
    let made =
        prepare_new_account_vault(&SecretString::from(PASSWORD), &account(), kdf, NOW).unwrap();
    let mut v = VaultService::new(Store::open_in_memory().unwrap());
    v.create_account_vault(made.prepared, &account_record())
        .unwrap();
    v
}

fn item(title: &str, user: &str, pw: &str, url: &str, totp: Option<&str>) -> ItemInput {
    ItemInput {
        item_type: ItemType::Login,
        title: title.into(),
        username: Some(user.into()),
        urls: vec![UrlRule {
            url: url.into(),
            match_type: MatchType::Domain,
        }],
        password: SecretUpdate::Set(SecretString::from(pw)),
        totp: totp.map_or(SecretUpdate::Keep, |t| {
            SecretUpdate::Set(SecretString::from(t))
        }),
        notes: SecretUpdate::Set(SecretString::from("login notes stay home")),
        content: SecretUpdate::Keep,
    }
}

fn build_fixture(writer: Option<()>) -> Fixture {
    let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
    let mut v = new_account_vault(kdf);
    v.update_settings(Settings {
        browser_integration: true,
        ..Settings::default()
    })
    .unwrap();
    let staged = v
        .stage_create(
            item(
                "GitHub",
                "octo",
                "gh-password",
                "https://github.com",
                Some("JBSWY3DPEHPK3PXP"),
            ),
            NOW,
        )
        .unwrap();
    let github = v.commit_write(staged, 1).unwrap().unwrap().id;
    let staged = v
        .stage_create(
            item(
                "Bank",
                "alice",
                "bank-password",
                "https://bank.example",
                None,
            ),
            NOW,
        )
        .unwrap();
    let bank = v.commit_write(staged, 2).unwrap().unwrap().id;
    let staged = v
        .stage_create(
            ItemInput {
                item_type: ItemType::SecureNote,
                title: "Recovery codes".into(),
                username: None,
                urls: vec![],
                password: SecretUpdate::Keep,
                totp: SecretUpdate::Keep,
                notes: SecretUpdate::Keep,
                content: SecretUpdate::Set(SecretString::from("note body")),
            },
            NOW,
        )
        .unwrap();
    let note = v.commit_write(staged, 3).unwrap().unwrap().id;

    let vault = Arc::new(Mutex::new(v));
    let locks = Arc::new(AtomicUsize::new(0));
    let changes = Arc::new(AtomicUsize::new(0));
    let (v2, l2, c2) = (vault.clone(), locks.clone(), changes.clone());
    let on_lock = move || {
        v2.lock().unwrap().lock();
        l2.fetch_add(1, Ordering::SeqCst);
    };
    let on_change = move || {
        c2.fetch_add(1, Ordering::SeqCst);
    };
    let bridge = match writer {
        // No writer: writes have nowhere to go, which is what an offline
        // device is.
        None => Bridge::with_change_hook(vault.clone(), on_lock, on_change),
        // A server that accepts everything, handing out revisions in order.
        Some(()) => {
            let v3 = vault.clone();
            let next = AtomicUsize::new(100);
            Bridge::with_writer(vault.clone(), on_lock, on_change, move |staged| {
                let revision = next.fetch_add(1, Ordering::SeqCst) as i64;
                v3.lock()
                    .map_err(|_| ErrorCode::Internal)?
                    .commit_write(staged, revision)
                    .map(|_| ())
                    .map_err(|_| ErrorCode::Internal)
            })
        }
    };
    Fixture {
        vault,
        bridge,
        locks,
        changes,
        github,
        bank,
        note,
    }
}

/// A device with no server session: staged writes have nowhere to go.
fn fixture() -> Fixture {
    build_fixture(None)
}

/// A device whose server accepts every write.
fn online_fixture() -> Fixture {
    build_fixture(Some(()))
}

fn request(id: u32, req: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"v": 1, "id": id, "request": req})).unwrap()
}

/// Send one request and return the response as JSON.
fn call(f: &Fixture, req: serde_json::Value) -> serde_json::Value {
    let out = f.bridge.handle_frame(&request(1, req));
    serde_json::from_slice(&out.to_bytes().unwrap()).unwrap()
}

fn error_code(resp: &serde_json::Value) -> Option<&str> {
    resp["error"]["code"].as_str()
}

fn fill(f: &Fixture, id: Uuid, url: &str) -> serde_json::Value {
    call(
        f,
        serde_json::json!({"type": "fill_item", "itemId": id, "url": url}),
    )
}

fn totp(f: &Fixture, id: Uuid, url: &str) -> serde_json::Value {
    call(
        f,
        serde_json::json!({"type": "get_totp", "itemId": id, "url": url}),
    )
}

fn find(f: &Fixture, url: &str) -> serde_json::Value {
    call(f, serde_json::json!({"type": "find_matches", "url": url}))
}

#[test]
fn legitimate_flow_works() {
    let f = fixture();
    let status = call(&f, serde_json::json!({"type": "status"}));
    assert_eq!(status["result"]["state"], "unlocked");
    assert_eq!(status["result"]["vaultExists"], true);

    let m = find(&f, "https://github.com/login");
    let matches = m["result"]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["id"], f.github.to_string());
    assert_eq!(matches[0]["username"], "octo");
    assert_eq!(matches[0]["hasTotp"], true);

    let r = fill(&f, f.github, "https://github.com/login");
    assert_eq!(r["result"]["password"], "gh-password");
    assert_eq!(r["result"]["username"], "octo");

    let t = totp(&f, f.github, "https://github.com/login");
    assert_eq!(t["result"]["code"].as_str().unwrap().len(), 6);
}

/// Secret minimization: find_matches never carries passwords, TOTP secrets
/// or notes; fill_item carries no notes or TOTP.
#[test]
fn responses_carry_only_what_the_operation_needs() {
    let f = fixture();
    let m = find(&f, "https://github.com/").to_string();
    for s in ["gh-password", "JBSWY3DPEHPK3PXP", "login notes"] {
        assert!(!m.contains(s), "find_matches leaked {s}");
    }
    let r = fill(&f, f.github, "https://github.com/").to_string();
    assert!(!r.contains("JBSWY3DPEHPK3PXP"));
    assert!(!r.contains("login notes"));
    let t = totp(&f, f.github, "https://github.com/").to_string();
    assert!(!t.contains("JBSWY3DPEHPK3PXP"));
    assert!(!t.contains("gh-password"));
}

/// A1: asking for the github.com login from another site is denied.
#[test]
fn a1_wrong_origin_denied() {
    for url in [
        "https://evil.com/",
        "https://github.com.evil.com/",
        "https://evilgithub.com/",
        "https://github-login.example.com/",
        "http://github.com/",
        "https://github.com@evil.com/",
        "https://evil.com/?next=https://github.com",
        "javascript:alert(1)",
        "not a url",
    ] {
        // Fresh fixture per URL so the rate limiter does not mask the result.
        let f = fixture();
        assert_eq!(
            error_code(&fill(&f, f.github, url)),
            Some("denied"),
            "{url}"
        );
        assert_eq!(
            error_code(&totp(&f, f.github, url)),
            Some("denied"),
            "{url}"
        );
        let m = find(&f, url);
        assert_eq!(m["result"]["matches"], serde_json::json!([]), "{url}");
    }
}

/// A2: an arbitrary item ID is only served on a page that item is saved for.
#[test]
fn a2_arbitrary_item_id_denied() {
    let f = fixture();
    // Another site's item.
    assert_eq!(
        error_code(&fill(&f, f.bank, "https://github.com/")),
        Some("denied")
    );
    // A secure note, from any page.
    assert_eq!(
        error_code(&fill(&f, f.note, "https://github.com/")),
        Some("denied")
    );
    // IDs that do not exist look exactly like "not for this site".
    assert_eq!(
        error_code(&fill(&f, Uuid::new_v4(), "https://github.com/")),
        Some("denied")
    );
    assert_eq!(
        error_code(&totp(&f, Uuid::new_v4(), "https://github.com/")),
        Some("denied")
    );
    // Item without TOTP: indistinguishable from the cases above.
    assert_eq!(
        error_code(&totp(&f, f.bank, "https://bank.example/")),
        Some("denied")
    );
}

/// A3: a locked vault answers status and nothing else.
#[test]
fn a3_locked_vault_refuses() {
    let f = fixture();
    f.vault.lock().unwrap().lock();
    assert_eq!(
        call(&f, serde_json::json!({"type": "status"}))["result"]["state"],
        "locked"
    );
    assert_eq!(error_code(&find(&f, "https://github.com/")), Some("locked"));
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("locked")
    );
    assert_eq!(
        error_code(&totp(&f, f.github, "https://github.com/")),
        Some("locked")
    );
}

#[test]
fn lock_request_locks_vault() {
    let f = fixture();
    let r = call(&f, serde_json::json!({"type": "lock"}));
    assert_eq!(r["result"]["type"], "lock");
    assert_eq!(f.locks.load(Ordering::SeqCst), 1);
    assert!(!f.vault.lock().unwrap().is_unlocked());
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("locked")
    );
}

#[test]
fn integration_switch_is_enforced() {
    // Off is the default for a new vault.
    let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
    let fresh = new_account_vault(kdf);
    assert!(!fresh.settings().unwrap().browser_integration);

    let f = fixture();
    {
        let mut v = f.vault.lock().unwrap();
        let s = v.settings().unwrap();
        v.update_settings(Settings {
            browser_integration: false,
            ..s
        })
        .unwrap();
    }
    assert_eq!(
        error_code(&find(&f, "https://github.com/")),
        Some("integration_disabled")
    );
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("integration_disabled")
    );
    assert_eq!(
        error_code(&totp(&f, f.github, "https://github.com/")),
        Some("integration_disabled")
    );
}

#[test]
fn secret_requests_are_rate_limited() {
    let f = fixture();
    let mut limited = false;
    for _ in 0..20 {
        let r = fill(&f, f.github, "https://github.com/");
        if error_code(&r) == Some("rate_limited") {
            limited = true;
            break;
        }
        assert!(r["result"]["password"].is_string());
    }
    assert!(limited);
    // Denied requests also consume the budget, so guessing is limited too.
    let f = fixture();
    for _ in 0..10 {
        fill(&f, Uuid::new_v4(), "https://github.com/");
    }
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("rate_limited")
    );
}

/// A1 for frames: a login frame is only served if its top page matches too.
#[test]
fn a1_frame_needs_matching_top_page() {
    let f = fixture();
    let url = "https://github.com/login";
    let find_in = |top: &str| {
        call(
            &f,
            serde_json::json!({"type": "find_matches", "url": url, "topUrl": top}),
        )
    };
    assert_eq!(
        find_in("https://evil.com/")["result"]["matches"],
        serde_json::json!([])
    );
    assert_eq!(
        find_in("https://github.com/")["result"]["matches"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let r = call(
        &f,
        serde_json::json!({"type": "fill_item", "itemId": f.github, "url": url, "topUrl": "https://evil.com/"}),
    );
    assert_eq!(error_code(&r), Some("denied"));
    let r = call(
        &f,
        serde_json::json!({"type": "get_totp", "itemId": f.github, "url": url, "topUrl": "https://evil.com/"}),
    );
    assert_eq!(error_code(&r), Some("denied"));
}

#[test]
fn generate_password_uses_core_generator() {
    let f = fixture();
    let a = call(&f, serde_json::json!({"type": "generate_password"}));
    let b = call(&f, serde_json::json!({"type": "generate_password"}));
    let (a, b) = (
        a["result"]["password"].as_str().unwrap(),
        b["result"]["password"].as_str().unwrap(),
    );
    assert_eq!(a.chars().count(), 24);
    assert_ne!(a, b);
    f.vault.lock().unwrap().lock();
    assert_eq!(
        error_code(&call(&f, serde_json::json!({"type": "generate_password"}))),
        Some("locked")
    );
}

fn check(f: &Fixture, url: &str, user: Option<&str>, pw: &str) -> serde_json::Value {
    call(
        f,
        serde_json::json!({"type": "check_login", "url": url, "username": user, "password": pw}),
    )
}

fn save(
    f: &Fixture,
    url: &str,
    user: Option<&str>,
    pw: &str,
    id: Option<Uuid>,
) -> serde_json::Value {
    call(
        f,
        serde_json::json!({"type": "save_login", "url": url, "username": user, "password": pw, "itemId": id}),
    )
}

#[test]
fn save_login_flow() {
    let f = fixture();
    let gh = "https://github.com/session";
    let r = check(&f, gh, Some("octo"), "gh-password");
    assert_eq!(
        r["result"],
        serde_json::json!({"type": "check_login", "action": "unchanged", "itemId": null})
    );
    let r = check(&f, gh, Some("octo"), "rotated");
    assert_eq!(r["result"]["action"], "update");
    assert_eq!(r["result"]["itemId"], f.github.to_string());
    assert!(
        !r.to_string().contains("gh-password"),
        "check_login never returns passwords"
    );

    // Without a server session the save has nowhere to go, and the
    // extension is told "offline" rather than a generic internal error, so
    // it can say something accurate.
    let r = save(&f, gh, Some("octo"), "rotated", Some(f.github));
    assert!(r["result"].is_null());
    assert_eq!(error_code(&r), Some("offline"));
    assert_eq!(fill(&f, f.github, gh)["result"]["password"], "gh-password");
    assert_eq!(f.changes.load(Ordering::SeqCst), 0);
}

/// The whole path, with a server that accepts: the browser's save reaches
/// the vault, and only the password changes.
#[test]
fn save_login_stores_the_password_when_the_server_accepts() {
    let f = online_fixture();
    let gh = "https://github.com/session";

    let r = save(&f, gh, Some("octo"), "rotated", Some(f.github));
    assert_eq!(r["result"]["type"], "save_login");
    assert_eq!(r["result"]["itemId"], f.github.to_string());
    assert_eq!(fill(&f, f.github, gh)["result"]["password"], "rotated");
    assert_eq!(f.changes.load(Ordering::SeqCst), 1);
    // The old password is recoverable, as it is for a change made in the app.
    assert_eq!(
        f.vault
            .lock()
            .unwrap()
            .password_history(&f.github)
            .unwrap()
            .len(),
        1
    );

    // A new site is saved for that site only.
    let r = save(&f, "https://new.example/login", Some("me"), "pw", None);
    let new_id: Uuid = r["result"]["itemId"].as_str().unwrap().parse().unwrap();
    assert_eq!(f.changes.load(Ordering::SeqCst), 2);
    assert_eq!(
        fill(&f, new_id, "https://new.example/")["result"]["password"],
        "pw"
    );
    assert_eq!(
        error_code(&fill(&f, new_id, "https://github.com/")),
        Some("denied")
    );
}

/// A2 for writes: the extension cannot overwrite another site's login.
#[test]
fn a2_save_login_cannot_touch_other_sites() {
    let f = fixture();
    for (url, id) in [
        ("https://evil.com/", f.github),
        ("https://github.com/", f.bank),
        ("https://github.com/", f.note),
        ("https://github.com/", Uuid::new_v4()),
    ] {
        assert_eq!(
            error_code(&save(&f, url, None, "pwned", Some(id))),
            Some("denied"),
            "{url}"
        );
    }
    assert_eq!(
        fill(&f, f.bank, "https://bank.example/")["result"]["password"],
        "bank-password"
    );
    assert_eq!(f.changes.load(Ordering::SeqCst), 0);

    // Locked and integration-off vaults refuse writes too.
    f.vault.lock().unwrap().lock();
    assert_eq!(
        error_code(&save(&f, "https://new.example/", None, "x", None)),
        Some("locked")
    );
    assert_eq!(
        error_code(&check(&f, "https://github.com/", None, "x")),
        Some("locked")
    );
}

/// A flood of password changes cannot push the real password out of the
/// item's history: one browser-initiated change per item per interval. The
/// limiter runs before the core, so it still engages even though every save
/// is refused for lack of a server session (spec 2026-09-20 §8.4).
#[test]
fn password_updates_are_limited_per_item() {
    let f = fixture();
    let url = "https://github.com/";
    assert!(save(&f, url, None, "one", Some(f.github))["result"].is_null());
    assert_eq!(
        error_code(&save(&f, url, None, "two", Some(f.github))),
        Some("rate_limited")
    );
    assert_eq!(fill(&f, f.github, url)["result"]["password"], "gh-password");
    // Adding new logins is not affected by the per-item limit.
    assert!(save(&f, "https://new.example/", None, "x", None)["result"].is_null());
}

#[test]
fn save_requests_share_the_secret_rate_limit() {
    let f = fixture();
    for _ in 0..10 {
        check(&f, "https://github.com/", Some("octo"), "guess");
    }
    assert_eq!(
        error_code(&check(&f, "https://github.com/", Some("octo"), "guess")),
        Some("rate_limited")
    );
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("rate_limited")
    );
}

/// A5 at the handler: unknown commands and malformed messages get an error
/// response, never a panic.
#[test]
fn a5_unknown_and_malformed_rejected() {
    let f = fixture();
    for bytes in [
        br#"{"v":1,"id":1,"request":{"type":"unlock","password":"x"}}"#.to_vec(),
        br#"{"v":1,"id":1,"request":{"type":"list_items"}}"#.to_vec(),
        br#"{"v":1,"id":1,"request":{"type":"reveal","itemId":"x"}}"#.to_vec(),
        b"\xff\xfe\x00garbage".to_vec(),
        Vec::new(),
    ] {
        let out: serde_json::Value =
            serde_json::from_slice(&f.bridge.handle_frame(&bytes).to_bytes().unwrap()).unwrap();
        assert!(out["error"]["code"].is_string());
        assert!(out.get("result").is_none());
    }
}

const CHAL: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc";

fn create_passkey(f: &Fixture, url: &str, item: Option<Uuid>) -> serde_json::Value {
    call(
        f,
        serde_json::json!({
            "type": "passkey_create", "url": url, "rpId": "github.com", "challenge": CHAL,
            "userHandle": "AQ", "userName": "octo", "displayName": null, "itemId": item,
            "conditional": false,
        }),
    )
}

#[test]
fn passkey_create_then_get_through_the_bridge() {
    let f = online_fixture();
    let check = call(
        &f,
        serde_json::json!({
            "type": "check_passkey_create", "url": "https://github.com/", "rpId": "github.com",
            "userName": "octo", "excludeCredentials": [], "conditional": false,
        }),
    );
    assert_eq!(check["result"]["excluded"], false);
    assert_eq!(
        check["result"]["candidates"][0]["itemId"],
        f.github.to_string()
    );

    let before = f.changes.load(Ordering::SeqCst);
    let created = create_passkey(&f, "https://github.com/", Some(f.github));
    let cred = created["result"]["credentialId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(created["result"]["publicKeyAlgorithm"], -7);
    assert!(created["result"].get("privateKey").is_none());
    assert_eq!(f.changes.load(Ordering::SeqCst), before + 1);

    let found = call(
        &f,
        serde_json::json!({
            "type": "find_passkeys", "url": "https://github.com/", "rpId": "github.com", "allowCredentials": [],
        }),
    );
    assert_eq!(found["result"]["passkeys"][0]["credentialId"], cred);

    let signed = call(
        &f,
        serde_json::json!({
            "type": "passkey_get", "itemId": f.github, "credentialId": cred,
            "url": "https://github.com/", "rpId": "github.com", "challenge": CHAL,
        }),
    );
    assert!(signed["result"]["signature"].as_str().unwrap().len() > 60);
}

#[test]
fn passkey_attacks_through_the_bridge() {
    let f = online_fixture();
    let created = create_passkey(&f, "https://github.com/", Some(f.github));
    let cred = created["result"]["credentialId"]
        .as_str()
        .unwrap()
        .to_owned();
    // A1: github.com's passkey requested from evil.com.
    let r = call(
        &f,
        serde_json::json!({
            "type": "passkey_get", "itemId": f.github, "credentialId": cred,
            "url": "https://evil.com/", "rpId": "github.com", "challenge": CHAL,
        }),
    );
    assert_eq!(error_code(&r), Some("denied"));
    // A2: unknown item IDs look exactly like a denial.
    let r = call(
        &f,
        serde_json::json!({
            "type": "passkey_get", "itemId": Uuid::new_v4(), "credentialId": cred,
            "url": "https://github.com/", "rpId": "github.com", "challenge": CHAL,
        }),
    );
    assert_eq!(error_code(&r), Some("denied"));
    // Creating on evil.com for github.com.
    let r = create_passkey(&f, "https://evil.com/", None);
    assert_eq!(error_code(&r), Some("denied"));
    // A3: locked.
    f.vault.lock().unwrap().lock();
    let r = call(
        &f,
        serde_json::json!({
            "type": "find_passkeys", "url": "https://github.com/", "rpId": "github.com", "allowCredentials": [],
        }),
    );
    assert_eq!(error_code(&r), Some("locked"));
}

#[test]
fn passkey_create_offline_is_refused_and_nothing_is_stored() {
    let f = fixture();
    let r = create_passkey(&f, "https://github.com/", Some(f.github));
    assert_eq!(error_code(&r), Some("offline"));
    assert!(
        !f.vault
            .lock()
            .unwrap()
            .get_item(&f.github)
            .unwrap()
            .has_passkey
    );
}

#[test]
fn passkey_upgrade_through_the_bridge() {
    let f = online_fixture();
    let check = |url: &str, rp: &str| {
        call(
            &f,
            serde_json::json!({
                "type": "check_passkey_create", "url": url, "rpId": rp,
                "userName": "octo", "excludeCredentials": [], "conditional": true,
            }),
        )
    };
    let create = |url: &str, rp: &str, item: Uuid| {
        call(
            &f,
            serde_json::json!({
                "type": "passkey_create", "url": url, "rpId": rp, "challenge": CHAL,
                "userHandle": "AQ", "userName": "octo", "displayName": null, "itemId": item,
                "conditional": true,
            }),
        )
    };
    // No fill yet: nothing, and a silent create is refused.
    assert_eq!(
        check("https://github.com/", "github.com")["result"]["upgrade"]["kind"],
        "none"
    );
    assert_eq!(
        error_code(&create("https://github.com/", "github.com", f.github)),
        Some("denied")
    );
    // A fill of the GitHub login.
    let filled = fill(&f, f.github, "https://github.com/login");
    assert!(filled["result"].is_object());
    assert_eq!(
        check("https://github.com/", "github.com")["result"]["upgrade"]["kind"],
        "auto"
    );
    // A1: another site gets nothing and cannot create.
    assert!(check("https://evil.com/", "github.com")["error"].is_object());
    assert_eq!(
        error_code(&create("https://evil.com/", "evil.com", f.github)),
        Some("denied")
    );
    // A2: another item.
    assert_eq!(
        error_code(&create("https://github.com/", "github.com", f.bank)),
        Some("denied")
    );
    // Allowed.
    assert!(create("https://github.com/", "github.com", f.github)["result"].is_object());
    // A3: locked.
    f.vault.lock().unwrap().lock();
    assert_eq!(
        error_code(&check("https://github.com/", "github.com")),
        Some("locked")
    );
}

#[test]
fn passkey_status_through_the_bridge() {
    let f = online_fixture();
    let status = |url: &str| {
        call(
            &f,
            serde_json::json!({"type": "passkey_status", "url": url}),
        )
    };
    assert_eq!(status("https://github.com/")["result"]["hasPasskey"], false);
    f.vault.lock().unwrap().lock();
    assert_eq!(error_code(&status("https://github.com/")), Some("locked"));
}

// ------------------------------------------------------------------ socket

#[cfg(unix)]
mod socket {
    use super::*;
    use std::io::Write;

    fn serve(f: &Fixture) -> (tempfile::TempDir, Endpoint) {
        let dir = tempfile::tempdir().unwrap();
        let ep = Endpoint::at(dir.path().join("hk").join("bridge.sock"));
        f.bridge.serve(&ep).unwrap();
        (dir, ep)
    }

    fn roundtrip(
        stream: &mut interprocess::local_socket::Stream,
        bytes: &[u8],
    ) -> serde_json::Value {
        write_frame(stream, bytes, usize::MAX).unwrap();
        let resp = read_frame(stream, MAX_RESPONSE_BYTES).unwrap().unwrap();
        serde_json::from_slice(&resp).unwrap()
    }

    #[test]
    fn end_to_end_over_socket() {
        let f = fixture();
        let (_dir, ep) = serve(&f);
        let mut s = ep.connect().unwrap();
        let r = roundtrip(
            &mut s,
            &request(
                5,
                serde_json::json!({"type":"fill_item","itemId":f.github,"url":"https://github.com/"}),
            ),
        );
        assert_eq!(r["id"], 5);
        assert_eq!(r["result"]["password"], "gh-password");
    }

    /// A5 on the socket: malformed input is answered and the connection
    /// keeps working.
    #[test]
    fn a5_malformed_message_does_not_break_connection() {
        let f = fixture();
        let (_dir, ep) = serve(&f);
        let mut s = ep.connect().unwrap();
        let r = roundtrip(&mut s, b"{not json");
        assert_eq!(r["error"]["code"], "malformed");
        let r = roundtrip(&mut s, &request(2, serde_json::json!({"type":"status"})));
        assert_eq!(r["result"]["state"], "unlocked");
    }

    /// A6: an oversized frame is refused by its length header alone and the
    /// connection is closed.
    #[test]
    fn a6_oversized_message_rejected() {
        let f = fixture();
        let (_dir, ep) = serve(&f);
        let mut s = ep.connect().unwrap();
        s.write_all(&(64u32 * 1024 * 1024).to_ne_bytes()).unwrap();
        s.flush().unwrap();
        let resp = read_frame(&mut s, MAX_RESPONSE_BYTES).unwrap().unwrap();
        let r: serde_json::Value = serde_json::from_slice(&resp).unwrap();
        assert_eq!(r["error"]["code"], "too_large");
        // Closed afterwards.
        assert!(read_frame(&mut s, MAX_RESPONSE_BYTES).unwrap().is_none());
        // And the server is still healthy.
        let mut s2 = ep.connect().unwrap();
        let r = roundtrip(&mut s2, &request(1, serde_json::json!({"type":"status"})));
        assert_eq!(r["result"]["state"], "unlocked");
    }

    #[test]
    fn lock_event_is_pushed() {
        let f = fixture();
        let (_dir, ep) = serve(&f);
        let mut s = ep.connect().unwrap();
        // Make sure the connection is registered before notifying.
        roundtrip(&mut s, &request(1, serde_json::json!({"type":"status"})));
        f.bridge.notify(Event::Locked {});
        let ev = read_frame(&mut s, MAX_RESPONSE_BYTES).unwrap().unwrap();
        assert_eq!(&*ev, br#"{"v":1,"event":{"type":"locked"}}"#);
    }

    #[test]
    fn connection_limit_enforced() {
        let f = fixture();
        let (_dir, ep) = serve(&f);
        let mut open = Vec::new();
        for i in 0..havenkeys_bridge::MAX_CONNECTIONS {
            let mut s = ep.connect().unwrap();
            roundtrip(
                &mut s,
                &request(i as u32, serde_json::json!({"type":"status"})),
            );
            open.push(s);
        }
        let mut extra = ep.connect().unwrap();
        // The server closes the extra connection without answering.
        let _ = write_frame(
            &mut extra,
            &request(99, serde_json::json!({"type":"status"})),
            usize::MAX,
        );
        assert!(!matches!(
            read_frame(&mut extra, MAX_RESPONSE_BYTES),
            Ok(Some(_))
        ));
        drop(open);
        // Slots are released when clients disconnect.
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(f.bridge.connection_count(), 0);
    }

    #[test]
    fn second_instance_refused_and_stale_socket_reclaimed() {
        let f = fixture();
        let (dir, ep) = serve(&f);
        let other = fixture();
        let err = other.bridge.serve(&ep).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);

        // A leftover socket file with nobody listening is replaced.
        let stale_dir = dir.path().join("stale");
        std::fs::create_dir(&stale_dir).unwrap();
        std::fs::set_permissions(
            &stale_dir,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let stale = stale_dir.join("bridge.sock");
        drop(std::os::unix::net::UnixListener::bind(&stale).unwrap());
        assert!(stale.exists());
        let ep2 = Endpoint::at(stale);
        other.bridge.serve(&ep2).unwrap();
        let mut s = ep2.connect().unwrap();
        let r = roundtrip(&mut s, &request(1, serde_json::json!({"type":"status"})));
        assert_eq!(r["result"]["state"], "unlocked");
    }

    #[test]
    fn refuses_non_private_directory() {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture();
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("shared");
        std::fs::create_dir(&shared).unwrap();
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();
        let ep = Endpoint::at(shared.join("bridge.sock"));
        assert_eq!(
            f.bridge.serve(&ep).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        // Clients refuse it too.
        assert!(ep.connect().is_err());

        // A symlink to a private directory is not accepted either.
        let private = dir.path().join("private");
        std::fs::create_dir(&private).unwrap();
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&private, &link).unwrap();
        let ep = Endpoint::at(link.join("bridge.sock"));
        assert_eq!(
            f.bridge.serve(&ep).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
}

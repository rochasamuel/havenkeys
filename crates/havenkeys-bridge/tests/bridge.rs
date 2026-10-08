//! Bridge security tests: the attacks in docs/threat-model.md §5, driven
//! through the real request parser and, where it matters, a real socket.

use havenkeys_bridge::Bridge;
use havenkeys_core::account::{AccountRef, NormalizedEmail};
use havenkeys_core::card::{CardExpiry, CardInput};
use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
use havenkeys_core::generator::GeneratorOptions;
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, Settings, UrlRule};
use havenkeys_core::sso::{SignInWith, SsoProvider};
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
    opened: Arc<Mutex<Vec<Uuid>>>,
    /// How many times `show_unlock` raised the desktop window.
    shown: Arc<AtomicUsize>,
    github: Uuid,
    bank: Uuid,
    note: Uuid,
    typeform: Uuid,
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
        tags: None,
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
        auto_sign_in: None,
        sign_in_with: None,
        identity: None,
        card: None,
        sections: None,
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
            ItemInput {
                tags: Some(vec!["Staging".into()]),
                ..item(
                    "GitHub",
                    "octo",
                    "gh-password",
                    "https://github.com",
                    Some("JBSWY3DPEHPK3PXP"),
                )
            },
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
                tags: None,
                item_type: ItemType::SecureNote,
                title: "Recovery codes".into(),
                username: None,
                urls: vec![],
                password: SecretUpdate::Keep,
                totp: SecretUpdate::Keep,
                notes: SecretUpdate::Keep,
                content: SecretUpdate::Set(SecretString::from("note body")),
                auto_sign_in: None,
                sign_in_with: None,
                identity: None,
                card: None,
                sections: None,
            },
            NOW,
        )
        .unwrap();
    let note = v.commit_write(staged, 3).unwrap().unwrap().id;
    let staged = v
        .stage_create(
            ItemInput {
                tags: None,
                item_type: ItemType::Login,
                title: "Typeform".into(),
                username: None,
                urls: vec![UrlRule {
                    url: "https://typeform.com".into(),
                    match_type: MatchType::Domain,
                }],
                password: SecretUpdate::Keep,
                totp: SecretUpdate::Keep,
                notes: SecretUpdate::Keep,
                content: SecretUpdate::Keep,
                auto_sign_in: None,
                sign_in_with: Some(SignInWith {
                    provider: SsoProvider::Google,
                    account: Some("me@gmail.com".into()),
                }),
                identity: None,
                card: None,
                sections: None,
            },
            NOW,
        )
        .unwrap();
    let typeform = v.commit_write(staged, 4).unwrap().unwrap().id;

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
    let opened = Arc::new(Mutex::new(Vec::new()));
    let o2 = opened.clone();
    bridge.set_open_item_hook(move |id| o2.lock().unwrap().push(id));
    let shown = Arc::new(AtomicUsize::new(0));
    let s2 = shown.clone();
    bridge.set_show_unlock_hook(move || {
        s2.fetch_add(1, Ordering::SeqCst);
    });

    Fixture {
        vault,
        bridge,
        locks,
        changes,
        opened,
        shown,
        github,
        bank,
        note,
        typeform,
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

// A tagged login's suggestion carries its tags; nothing else about it changes.
#[test]
fn suggestions_carry_the_logins_tags() {
    let f = fixture();
    let m = find(&f, "https://github.com/login");
    let matches = m["result"]["matches"].as_array().unwrap();
    assert_eq!(matches[0]["tags"], serde_json::json!(["Staging"]));
    let m = find(&f, "https://bank.example/");
    assert_eq!(m["result"]["matches"][0]["tags"], serde_json::json!([]));
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

/// Final review (tags): 50 logins with the longest titles, usernames and 20
/// tags each made a find_matches answer over MAX_RESPONSE_BYTES, so the
/// frame could not be written and the extension lost its connection. Tags
/// now go, last match first, until it fits.
#[test]
fn the_largest_find_matches_answer_still_fits_one_frame() {
    let f = fixture();
    // Four bytes a character, and no case to fold: Rust stores them as given.
    let wide = |n: usize| "\u{1D54F}".repeat(n);
    let tags: Vec<String> = (0..20u32)
        .map(|i| format!("{}{}", wide(31), char::from_u32(0x1D400 + i).unwrap()))
        .collect();
    {
        let mut v = f.vault.lock().unwrap();
        for i in 0..MAX_MATCHES {
            let input = ItemInput {
                tags: Some(tags.clone()),
                ..item(
                    &format!("{}{:03}", wide(253), i),
                    &wide(512),
                    "pw",
                    "https://big.example",
                    None,
                )
            };
            let staged = v.stage_create(input, NOW).unwrap();
            v.commit_write(staged, 10 + i as i64).unwrap();
        }
    }
    let out = f.bridge.handle_frame(&request(
        1,
        serde_json::json!({"type": "find_matches", "url": "https://big.example/"}),
    ));
    let bytes = out.to_bytes().unwrap();
    assert!(bytes.len() <= MAX_RESPONSE_BYTES, "{} bytes", bytes.len());
    let mut wire = Vec::new();
    write_frame(&mut wire, &bytes, MAX_RESPONSE_BYTES).expect("the answer is framed");
    // The native host accepts it, and every login is still suggested.
    assert!(Outgoing::parse(&bytes).is_some());
    let resp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let matches = resp["result"]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), MAX_MATCHES);
    assert_eq!(matches[0]["tags"].as_array().unwrap().len(), 20);
    assert_eq!(matches[MAX_MATCHES - 1]["tags"], serde_json::json!([]));
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

/// A2 variant: a trashed login is answered exactly like an unknown ID, even
/// from the origin it was saved for.
#[test]
fn a2_a_trashed_login_is_denied_like_an_unknown_id() {
    let f = fixture();
    {
        let mut v = f.vault.lock().unwrap();
        let staged = v.stage_trash(&f.github, NOW).unwrap();
        v.commit_write(staged, 99).unwrap();
    }
    let page = "https://github.com/";
    assert_eq!(error_code(&fill(&f, f.github, page)), Some("denied"));
    assert_eq!(error_code(&totp(&f, f.github, page)), Some("denied"));
    assert_eq!(
        fill(&f, f.github, page)["error"],
        fill(&f, Uuid::new_v4(), page)["error"],
        "same answer as an unknown ID"
    );
    assert_eq!(
        find(&f, page)["result"]["matches"],
        serde_json::json!([]),
        "not offered either"
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
fn every_page_request_is_refused_while_integration_is_off() {
    let off = || {
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
        f
    };
    let item = Uuid::from_u128(1);
    let url = "https://github.com/";
    let requests = [
        serde_json::json!({"type": "find_matches", "url": url}),
        serde_json::json!({"type": "fill_item", "itemId": item, "url": url}),
        serde_json::json!({"type": "get_totp", "itemId": item, "url": url}),
        serde_json::json!({"type": "generate_password"}),
        serde_json::json!({"type": "check_login", "url": url, "username": "octo", "password": "pw"}),
        serde_json::json!({"type": "save_login", "url": url, "username": "octo", "password": "pw", "itemId": null}),
        serde_json::json!({"type": "find_passkeys", "url": url, "rpId": "github.com", "allowCredentials": []}),
        serde_json::json!({"type": "passkey_get", "itemId": item, "credentialId": "AAAAAAAAAAAAAAAAAAAAAA", "url": url, "rpId": "github.com", "challenge": CHAL}),
        serde_json::json!({"type": "check_passkey_create", "url": url, "rpId": "github.com", "userName": "octo", "excludeCredentials": [], "conditional": false}),
        serde_json::json!({"type": "passkey_create", "url": url, "rpId": "github.com", "challenge": CHAL, "userHandle": "AQ", "userName": "octo", "displayName": null, "itemId": null, "conditional": false}),
        serde_json::json!({"type": "passkey_status", "url": url}),
        serde_json::json!({"type": "open_item", "itemId": item, "url": url}),
        serde_json::json!({"type": "find_identity", "url": url}),
        serde_json::json!({"type": "fill_identity", "url": url, "roles": ["firstName"], "documents": false}),
        serde_json::json!({"type": "open_identity", "url": url}),
        serde_json::json!({"type": "find_cards", "url": url}),
        serde_json::json!({"type": "fill_card", "itemId": item, "topUrl": url, "frames": [{"url": url, "roles": ["number"]}]}),
        serde_json::json!({"type": "save_card", "url": url, "number": "4111111111111111"}),
        serde_json::json!({"type": "start_sso", "itemId": item, "url": url}),
        serde_json::json!({"type": "check_sso", "url": url, "provider": "google", "account": null}),
        serde_json::json!({"type": "save_sso", "url": url, "provider": "google", "account": null, "itemId": null}),
    ];
    for req in requests {
        // A fresh bridge each time, so no request is rate limited first.
        let f = off();
        let before = f.changes.load(Ordering::SeqCst);
        let r = call(&f, req.clone());
        assert_eq!(error_code(&r), Some("integration_disabled"), "{req}");
        assert_eq!(f.changes.load(Ordering::SeqCst), before, "{req}");
        assert!(f.opened.lock().unwrap().is_empty(), "{req}");
    }
    // status and lock are not page requests.
    let f = off();
    assert_eq!(
        call(&f, serde_json::json!({"type": "status"}))["result"]["state"],
        "unlocked"
    );
    assert_eq!(
        call(&f, serde_json::json!({"type": "lock"}))["result"]["type"],
        "lock"
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

/// Give the fixture's vault a Visa with a CVV.
fn add_card(f: &Fixture) -> Uuid {
    let mut v = f.vault.lock().unwrap();
    let input = ItemInput {
        tags: None,
        item_type: ItemType::Card,
        title: "Visa".into(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: None,
        card: Some(CardInput {
            cardholder_name: Some(SecretString::from("Samuel Rocha")),
            brand: None,
            number: SecretUpdate::Set(SecretString::from("4111111111111111")),
            verification_number: SecretUpdate::Set(SecretString::from("123")),
            expiry: Some(CardExpiry {
                year: 2033,
                month: 11,
            }),
            notes: None,
        }),
        sections: None,
    };
    let staged = v.stage_create(input, NOW).unwrap();
    v.commit_write(staged, 60).unwrap().unwrap().id
}

fn fill_card(f: &Fixture, id: Uuid, top: &str, frames: serde_json::Value) -> serde_json::Value {
    call(
        f,
        serde_json::json!({"type": "fill_card", "itemId": id, "topUrl": top, "frames": frames}),
    )
}

#[test]
fn cards_are_found_and_filled_across_processor_frames() {
    let f = fixture();
    let id = add_card(&f);
    let found = call(
        &f,
        serde_json::json!({"type": "find_cards", "url": "https://shop.com/checkout"}),
    );
    assert_eq!(found["result"]["insecure"], false);
    assert_eq!(
        found["result"]["cards"],
        serde_json::json!([{"id": id, "title": "Visa", "brand": "visa", "last4": "1111", "expiry": "2033-11"}])
    );
    assert!(!found.to_string().contains("4111111111111111"));
    let r = fill_card(
        &f,
        id,
        "https://shop.com/checkout",
        serde_json::json!([
            {"url": "https://shop.com/checkout", "roles": ["cardholderName"]},
            {"url": "https://js.stripe.com/v3/elements-inner-card.html", "roles": ["number", "expiryMonth", "expiryYear", "verificationNumber"]}
        ]),
    );
    assert_eq!(
        r["result"]["frames"],
        serde_json::json!([
            {"values": [{"role": "cardholderName", "value": "Samuel Rocha"}]},
            {"values": [
                {"role": "number", "value": "4111111111111111"},
                {"role": "expiryMonth", "value": "11"},
                {"role": "expiryYear", "value": "2033"},
                {"role": "verificationNumber", "value": "123"}
            ]}
        ])
    );
}

/// Attacks: an http checkout, a look-alike processor host, and evil.com
/// framing a real checkout.
#[test]
fn cards_are_denied_to_insecure_and_foreign_frames() {
    let f = fixture();
    let id = add_card(&f);
    let http = call(
        &f,
        serde_json::json!({"type": "find_cards", "url": "http://shop.com/"}),
    );
    assert_eq!(
        http["result"],
        serde_json::json!({"type": "find_cards", "insecure": true, "cards": []})
    );
    let one = |url: &str| serde_json::json!([{"url": url, "roles": ["number"]}]);
    assert_eq!(
        error_code(&fill_card(
            &f,
            id,
            "http://shop.com/",
            one("http://shop.com/")
        )),
        Some("denied")
    );
    assert_eq!(
        error_code(&fill_card(
            &f,
            id,
            "https://shop.com/",
            one("https://js.stripe.com.evil.com/")
        )),
        Some("denied")
    );
    assert_eq!(
        error_code(&fill_card(
            &f,
            id,
            "https://evil.com/",
            one("https://shop.com/checkout")
        )),
        Some("denied")
    );
}

/// Look-alike processor frames are refused; the real host is accepted in
/// its normalized spellings.
#[test]
fn processor_frame_lookalikes_are_denied() {
    let f = fixture();
    let id = add_card(&f);
    let one = |url: &str| serde_json::json!([{"url": url, "roles": ["number"]}]);
    let top = "https://shop.com/";
    for bad in [
        "https://evilstripe.com/",
        "https://js.stripe.com@evil.com/",
        "https://js.\u{455}tripe.com/",
    ] {
        assert_eq!(
            error_code(&fill_card(&f, id, top, one(bad))),
            Some("denied"),
            "{bad}"
        );
    }
    for good in ["https://js.stripe.com:443/", "https://JS.STRIPE.COM/"] {
        let r = fill_card(&f, id, top, one(good));
        assert_eq!(
            r["result"]["frames"],
            serde_json::json!([{"values": [{"role": "number", "value": "4111111111111111"}]}]),
            "{good}"
        );
    }
}

#[test]
fn fill_card_answers_not_found_for_logins_and_unknown_ids() {
    let f = fixture();
    add_card(&f);
    let frames = serde_json::json!([{"url": "https://shop.com/", "roles": ["number"]}]);
    assert_eq!(
        error_code(&fill_card(
            &f,
            f.github,
            "https://shop.com/",
            frames.clone()
        )),
        Some("not_found")
    );
    assert_eq!(
        error_code(&fill_card(&f, Uuid::new_v4(), "https://shop.com/", frames)),
        Some("not_found")
    );
}

/// Regression: login requests never hand out a card.
#[test]
fn login_requests_never_return_a_card() {
    let f = fixture();
    let id = add_card(&f);
    assert_eq!(
        error_code(&fill(&f, id, "https://shop.com/")),
        Some("denied")
    );
    assert_eq!(
        error_code(&totp(&f, id, "https://shop.com/")),
        Some("denied")
    );
    let m = find(&f, "https://shop.com/");
    assert_eq!(m["result"]["matches"], serde_json::json!([]));
}

#[test]
fn save_card_writes_through_the_server_and_needs_it() {
    let offline = fixture();
    let req = serde_json::json!({"type": "save_card", "url": "https://shop.com/", "number": "4000 0566 5566 5556", "expiry": "2030-01"});
    assert_eq!(error_code(&call(&offline, req.clone())), Some("offline"));

    let f = online_fixture();
    let before = f.changes.load(Ordering::SeqCst);
    let r = call(&f, req);
    let id = r["result"]["itemId"].as_str().expect("saved").to_owned();
    assert_eq!(f.changes.load(Ordering::SeqCst), before + 1);
    let found = call(
        &f,
        serde_json::json!({"type": "find_cards", "url": "https://shop.com/"}),
    );
    assert!(found.to_string().contains(&id));

    let bad = call(
        &f,
        serde_json::json!({"type": "save_card", "url": "https://shop.com/", "number": "4111111111111112"}),
    );
    assert_eq!(error_code(&bad), Some("invalid_input"));
}

#[test]
fn card_requests_respect_lock_integration_and_rate_limits() {
    let f = fixture();
    let id = add_card(&f);
    let frames = serde_json::json!([{"url": "https://shop.com/", "roles": ["number"]}]);
    for _ in 0..10 {
        fill_card(&f, id, "https://shop.com/", frames.clone());
    }
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("rate_limited")
    );

    let g = fixture();
    add_card(&g);
    g.vault
        .lock()
        .unwrap()
        .update_settings(Settings {
            browser_integration: false,
            ..Settings::default()
        })
        .unwrap();
    let r = call(
        &g,
        serde_json::json!({"type": "find_cards", "url": "https://shop.com/"}),
    );
    assert_eq!(error_code(&r), Some("integration_disabled"));
    g.vault.lock().unwrap().lock();
    let r = call(
        &g,
        serde_json::json!({"type": "find_cards", "url": "https://shop.com/"}),
    );
    assert_eq!(error_code(&r), Some("locked"));
}

/// Rate classes: `find_cards` is a Lookup (60-burst, leaves the Secret
/// bucket alone); `fill_card` and `save_card` are Secret (burst of 10).
#[test]
fn card_requests_use_the_right_rate_class() {
    let find_req = serde_json::json!({"type": "find_cards", "url": "https://shop.com/"});

    let f = fixture();
    for _ in 0..30 {
        call(&f, find_req.clone());
    }
    // 30 lookups did not touch the Secret bucket.
    assert_ne!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("rate_limited")
    );
    let mut limited = false;
    for _ in 0..40 {
        limited |= error_code(&call(&f, find_req.clone())) == Some("rate_limited");
    }
    assert!(limited, "find_cards is limited at the Lookup burst");

    let f = fixture();
    let id = add_card(&f);
    let frames = serde_json::json!([{"url": "https://shop.com/", "roles": ["number"]}]);
    for _ in 0..10 {
        fill_card(&f, id, "https://shop.com/", frames.clone());
    }
    assert_eq!(
        error_code(&fill_card(&f, id, "https://shop.com/", frames)),
        Some("rate_limited")
    );
    // find_cards is not blocked by an exhausted Secret bucket.
    assert_ne!(
        error_code(&call(&f, find_req.clone())),
        Some("rate_limited")
    );

    let f = fixture();
    let save_req = serde_json::json!({"type": "save_card", "url": "https://shop.com/", "number": "4111111111111111"});
    for _ in 0..10 {
        call(&f, save_req.clone());
    }
    assert_eq!(error_code(&call(&f, save_req)), Some("rate_limited"));
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

fn open(f: &Fixture, id: Uuid, url: &str) -> serde_json::Value {
    call(
        f,
        serde_json::json!({"type": "open_item", "itemId": id, "url": url}),
    )
}

#[test]
fn open_item_opens_a_login_saved_for_the_page() {
    let f = fixture();
    let r = open(&f, f.github, "https://github.com/login");
    assert_eq!(r["result"], serde_json::json!({"type": "open_item"}));
    assert_eq!(*f.opened.lock().unwrap(), vec![f.github]);
}

#[test]
fn open_item_is_origin_bound() {
    for url in [
        "https://evil.com/",
        "https://github.com.evil.com/",
        "https://evilgithub.com/",
        "not a url",
    ] {
        let f = fixture();
        assert_eq!(
            error_code(&open(&f, f.github, url)),
            Some("denied"),
            "{url}"
        );
        assert!(f.opened.lock().unwrap().is_empty(), "{url}");
    }
    let f = fixture();
    assert_eq!(
        error_code(&open(&f, f.bank, "https://github.com/")),
        Some("denied")
    );
    assert_eq!(
        error_code(&open(&f, f.note, "https://github.com/")),
        Some("denied")
    );
    assert_eq!(
        error_code(&open(&f, Uuid::new_v4(), "https://github.com/")),
        Some("denied")
    );
    assert!(f.opened.lock().unwrap().is_empty());
}

#[test]
fn open_item_refuses_when_locked_or_disabled() {
    let f = fixture();
    f.vault.lock().unwrap().lock();
    assert_eq!(
        error_code(&open(&f, f.github, "https://github.com/")),
        Some("locked")
    );

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
        error_code(&open(&f, f.github, "https://github.com/")),
        Some("integration_disabled")
    );
    assert!(f.opened.lock().unwrap().is_empty());
}

#[test]
fn open_item_is_rate_limited_as_a_secret_request() {
    let f = fixture();
    let mut limited = false;
    for _ in 0..20 {
        if error_code(&open(&f, f.github, "https://github.com/")) == Some("rate_limited") {
            limited = true;
            break;
        }
    }
    assert!(limited);
    // It shares the Secret bucket with fill_item.
    let f = fixture();
    for _ in 0..10 {
        open(&f, Uuid::new_v4(), "https://github.com/");
    }
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("rate_limited")
    );
}

#[test]
fn open_item_without_a_hook_is_an_internal_error() {
    let f = fixture();
    let bare = Bridge::new(f.vault.clone(), || {});
    let r = bare.handle_frame(&request(
        1,
        serde_json::json!({
            "type": "open_item", "itemId": f.github, "url": "https://github.com/"
        }),
    ));
    let r: serde_json::Value = serde_json::from_slice(&r.to_bytes().unwrap()).unwrap();
    assert_eq!(error_code(&r), Some("internal"));
}

fn show_unlock(f: &Fixture) -> serde_json::Value {
    call(f, serde_json::json!({"type": "show_unlock"}))
}

#[test]
fn show_unlock_raises_the_window_while_locked() {
    let f = fixture();
    f.vault.lock().unwrap().lock();
    let r = show_unlock(&f);
    assert_eq!(r["result"], serde_json::json!({"type": "show_unlock"}));
    assert_eq!(f.shown.load(Ordering::SeqCst), 1);
    // It never touches the vault: still locked, nothing opened, no lock hook.
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("locked")
    );
    assert!(f.opened.lock().unwrap().is_empty());
    assert_eq!(f.locks.load(Ordering::SeqCst), 0);
}

#[test]
fn show_unlock_while_unlocked_just_raises_the_window() {
    let f = fixture();
    let r = show_unlock(&f);
    assert_eq!(r["result"], serde_json::json!({"type": "show_unlock"}));
    assert_eq!(f.shown.load(Ordering::SeqCst), 1);
    // The vault stays unlocked.
    assert!(fill(&f, f.github, "https://github.com/")["result"].is_object());
}

#[test]
fn show_unlock_rejects_extra_fields() {
    let f = fixture();
    let r = call(
        &f,
        serde_json::json!({"type": "show_unlock", "password": "hunter2"}),
    );
    assert_eq!(error_code(&r), Some("malformed"));
    assert_eq!(f.shown.load(Ordering::SeqCst), 0);
}

#[test]
fn show_unlock_is_rate_limited_as_a_secret_request() {
    let f = fixture();
    f.vault.lock().unwrap().lock();
    let mut limited = false;
    for _ in 0..20 {
        if error_code(&show_unlock(&f)) == Some("rate_limited") {
            limited = true;
            break;
        }
    }
    assert!(limited);
    assert!(f.shown.load(Ordering::SeqCst) <= 10);
    // It shares the Secret bucket with fill_item.
    let f = fixture();
    for _ in 0..10 {
        show_unlock(&f);
    }
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("rate_limited")
    );
}

#[test]
fn show_unlock_without_a_hook_is_an_internal_error() {
    let f = fixture();
    let bare = Bridge::new(f.vault.clone(), || {});
    let r = bare.handle_frame(&request(1, serde_json::json!({"type": "show_unlock"})));
    let r: serde_json::Value = serde_json::from_slice(&r.to_bytes().unwrap()).unwrap();
    assert_eq!(error_code(&r), Some("internal"));
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

#[test]
fn generate_password_follows_requested_options() {
    let f = fixture();
    let r = call(
        &f,
        serde_json::json!({"type": "generate_password", "options": {
            "length": 40, "uppercase": false, "lowercase": false, "digits": true, "symbols": false,
            "avoidAmbiguous": true,
        }}),
    );
    let pw = r["result"]["password"].as_str().unwrap();
    assert_eq!(pw.chars().count(), 40);
    assert!(pw
        .chars()
        .all(|c| c.is_ascii_digit() && c != '0' && c != '1'));
}

#[test]
fn generator_follows_the_desktop_generator_tab() {
    let f = fixture();
    {
        let mut v = f.vault.lock().unwrap();
        let s = v.settings().unwrap();
        v.update_settings(Settings {
            generator: GeneratorOptions {
                length: 12,
                uppercase: true,
                lowercase: false,
                digits: false,
                symbols: false,
                avoid_ambiguous: true,
            },
            ..s
        })
        .unwrap();
    }
    let r = call(&f, serde_json::json!({"type": "generator_options"}));
    assert_eq!(
        r["result"]["options"],
        serde_json::json!({"length": 12, "uppercase": true, "lowercase": false, "digits": false,
            "symbols": false, "avoidAmbiguous": true})
    );
    // No options: the saved policy.
    let r = call(&f, serde_json::json!({"type": "generate_password"}));
    let pw = r["result"]["password"].as_str().unwrap();
    assert_eq!(pw.chars().count(), 12);
    assert!(pw
        .chars()
        .all(|c| c.is_ascii_uppercase() && c != 'I' && c != 'O'));

    f.vault.lock().unwrap().lock();
    assert_eq!(
        error_code(&call(&f, serde_json::json!({"type": "generator_options"}))),
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
    let f = online_fixture();
    let url = "https://github.com/";
    assert!(error_code(&save(&f, url, None, "one", Some(f.github))).is_none());
    assert_eq!(
        error_code(&save(&f, url, None, "two", Some(f.github))),
        Some("rate_limited")
    );
    assert_eq!(fill(&f, f.github, url)["result"]["password"], "one");
    // Adding new logins is not affected by the per-item limit.
    assert!(error_code(&save(&f, "https://new.example/", None, "x", None)).is_none());
}

/// BR-2: a change that fails (no server) or is denied (wrong site) gives
/// the item's budget back, so the user's retry still goes through.
#[test]
fn failed_or_denied_password_updates_do_not_spend_the_item_budget() {
    let offline = fixture();
    let url = "https://github.com/";
    for _ in 0..3 {
        assert_eq!(
            error_code(&save(&offline, url, None, "one", Some(offline.github))),
            Some("offline")
        );
    }

    let f = online_fixture();
    for _ in 0..3 {
        assert!(error_code(&save(&f, "https://evil.com/", None, "x", Some(f.github))).is_some());
    }
    assert!(error_code(&save(&f, url, None, "one", Some(f.github))).is_none());
    assert_eq!(
        error_code(&save(&f, url, None, "two", Some(f.github))),
        Some("rate_limited")
    );
}

/// `save_sso` shares `save_login`'s per-item cooldown (server.rs checks both
/// request kinds under the same `allow_item_update` branch): a second
/// `save_sso` for the same item inside the interval is rate-limited, and so
/// is a `save_login` for that same item afterward, whichever request made
/// the first change.
#[test]
fn save_sso_shares_the_per_item_limiter() {
    let f = online_fixture();
    let url = "https://typeform.com/";
    let save_sso = |account: &str| {
        call(
            &f,
            serde_json::json!({"type": "save_sso", "url": url, "provider": "google", "account": account, "itemId": f.typeform}),
        )
    };
    assert!(error_code(&save_sso("one@gmail.com")).is_none());
    assert_eq!(error_code(&save_sso("two@gmail.com")), Some("rate_limited"));
    // A save_login for the same item right after still hits the cooldown
    // save_sso just started.
    assert_eq!(
        error_code(&save(&f, url, None, "pw", Some(f.typeform))),
        Some("rate_limited")
    );
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

fn start_sso(f: &Fixture, id: Uuid, url: &str) -> serde_json::Value {
    call(
        f,
        serde_json::json!({"type": "start_sso", "itemId": id, "url": url}),
    )
}

#[test]
fn sso_flow_returns_no_secrets() {
    let f = fixture();
    let m = find(&f, "https://typeform.com/login");
    let row = &m["result"]["matches"][0];
    assert_eq!(row["provider"], "google");
    assert_eq!(
        row["username"], "me@gmail.com",
        "the account stands in for a missing username"
    );
    let github_row = &find(&f, "https://github.com/")["result"]["matches"][0];
    assert!(github_row["provider"].is_null());

    let r = start_sso(&f, f.typeform, "https://typeform.com/login");
    assert_eq!(
        r["result"],
        serde_json::json!({"type": "start_sso", "provider": "google", "account": "me@gmail.com",
            "providerOrigins": ["https://accounts.google.com"], "autoChoose": true})
    );
}

/// A1/A2 for sign-in-with: another site, another item, a locked vault.
#[test]
fn start_sso_attacks_are_denied() {
    let f = fixture();
    for (url, id) in [
        ("https://evil.com/", f.typeform),
        ("https://typeform.com.evil.com/", f.typeform),
        ("https://github.com/", f.github), // a login without sign-in-with
        ("https://typeform.com/", Uuid::new_v4()),
        ("https://typeform.com/", f.note),
    ] {
        assert_eq!(error_code(&start_sso(&f, id, url)), Some("denied"), "{url}");
    }
    f.vault.lock().unwrap().lock();
    assert_eq!(
        error_code(&start_sso(&f, f.typeform, "https://typeform.com/")),
        Some("locked")
    );
}

#[test]
fn save_sso_flow() {
    let f = online_fixture();
    let r = call(
        &f,
        serde_json::json!({"type": "check_sso", "url": "https://typeform.com/", "provider": "google", "account": "ME@gmail.com"}),
    );
    assert_eq!(
        r["result"],
        serde_json::json!({"type": "check_sso", "action": "unchanged", "itemId": null})
    );
    // A new "Sign in with GitHub" is offered the vault's GitHub account
    // (the login saved for github.com), with no secret.
    let r = call(
        &f,
        serde_json::json!({"type": "check_sso", "url": "https://canva.com/", "provider": "github", "account": null}),
    );
    assert_eq!(
        r["result"],
        serde_json::json!({"type": "check_sso", "action": "add", "itemId": null, "accounts": ["octo"]})
    );
    let r = call(
        &f,
        serde_json::json!({"type": "check_sso", "url": "https://canva.com/", "provider": "apple", "account": null}),
    );
    assert_eq!(
        r["result"],
        serde_json::json!({"type": "check_sso", "action": "add", "itemId": null})
    );
    let r = call(
        &f,
        serde_json::json!({"type": "save_sso", "url": "https://canva.com/", "provider": "apple", "account": null, "itemId": null, "title": "Canva"}),
    );
    assert_eq!(r["result"]["type"], "save_sso");
    assert_eq!(f.changes.load(Ordering::SeqCst), 1);
    // Cannot retarget another site's login.
    let r = call(
        &f,
        serde_json::json!({"type": "save_sso", "url": "https://evil.com/", "provider": "google", "account": "x", "itemId": f.typeform}),
    );
    assert_eq!(error_code(&r), Some("denied"));
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

    /// Eight frames with long URLs (8 x ~4 KB) exceed `MAX_REQUEST_BYTES`
    /// (16 KiB). The limit is enforced on the length header by the frame
    /// reader, so the request is refused as `too_large` before it is read or
    /// parsed, with no panic and the server still healthy.
    #[test]
    fn fill_card_with_eight_long_frames_is_rejected_as_too_large() {
        let f = fixture();
        let id = add_card(&f);
        let (_dir, ep) = serve(&f);
        let long = format!("https://js.stripe.com/{}", "a".repeat(4000));
        let frames: Vec<_> = (0..8)
            .map(|_| serde_json::json!({"url": long, "roles": ["number"]}))
            .collect();
        let body = request(
            1,
            serde_json::json!({"type": "fill_card", "itemId": id, "topUrl": "https://shop.com/", "frames": frames}),
        );
        assert!(body.len() > MAX_REQUEST_BYTES);
        let mut s = ep.connect().unwrap();
        write_frame(&mut s, &body, usize::MAX).unwrap();
        let resp = read_frame(&mut s, MAX_RESPONSE_BYTES).unwrap().unwrap();
        let r: serde_json::Value = serde_json::from_slice(&resp).unwrap();
        assert_eq!(r["error"]["code"], "too_large");
        let mut s2 = ep.connect().unwrap();
        let r = roundtrip(&mut s2, &request(2, serde_json::json!({"type": "status"})));
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

    /// autoSubmit comes from the core: the vault setting AND the login's switch.
    #[test]
    fn fills_carry_auto_submit_from_the_core() {
        let f = fixture();
        let url = "https://github.com/login";
        assert_eq!(fill(&f, f.github, url)["result"]["autoSubmit"], true);
        assert_eq!(totp(&f, f.github, url)["result"]["autoSubmit"], true);
        {
            let mut v = f.vault.lock().unwrap();
            let s = v.settings().unwrap();
            v.update_settings(Settings {
                auto_sign_in: false,
                ..s
            })
            .unwrap();
        }
        assert_eq!(fill(&f, f.github, url)["result"]["autoSubmit"], false);
        assert_eq!(totp(&f, f.github, url)["result"]["autoSubmit"], false);
        // A wrong origin is still denied before anything else.
        assert_eq!(
            error_code(&fill(&f, f.github, "https://evil.com/")),
            Some("denied")
        );
    }
}

use havenkeys_core::identity::IdentityFields;

/// Give the fixture's vault its identity, with a name, postal code and CPF.
fn add_identity(f: &Fixture) -> Uuid {
    let mut v = f.vault.lock().unwrap();
    let staged = v
        .stage_identity_if_missing("user@example.com", NOW)
        .unwrap()
        .unwrap();
    let id = staged.item_id;
    v.commit_write(staged, 50).unwrap();
    let input = ItemInput {
        tags: None,
        item_type: ItemType::Identity,
        title: String::new(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: Some(IdentityFields {
            first_name: Some(SecretString::from("Samuel")),
            postal_code: Some(SecretString::from("71266-105")),
            cpf: Some(SecretString::from("123.456.789-00")),
            ..Default::default()
        }),
        card: None,
        sections: None,
    };
    let staged = v.stage_update(&id, input, NOW + 1).unwrap();
    v.commit_write(staged, 51).unwrap();
    id
}

fn fill_identity(
    f: &Fixture,
    url: &str,
    top: Option<&str>,
    roles: &[&str],
    documents: bool,
) -> serde_json::Value {
    let mut req = serde_json::json!({"type": "fill_identity", "url": url, "roles": roles, "documents": documents});
    if let Some(t) = top {
        req["topUrl"] = serde_json::json!(t);
    }
    call(f, req)
}

#[test]
fn identity_is_found_and_filled_by_role() {
    let f = fixture();
    add_identity(&f);
    let found = call(
        &f,
        serde_json::json!({"type": "find_identity", "url": "https://shop.com/"}),
    );
    assert_eq!(found["result"]["title"], "Samuel");
    let roles = found["result"]["roles"].as_array().unwrap();
    assert!(roles.contains(&serde_json::json!("cpf")), "names only");
    assert!(
        !found.to_string().contains("123.456"),
        "no values in find_identity"
    );

    let r = fill_identity(
        &f,
        "https://shop.com/",
        None,
        &["firstName", "postalCode"],
        false,
    );
    assert_eq!(
        r["result"]["values"],
        serde_json::json!([{"role": "firstName", "value": "Samuel"}, {"role": "postalCode", "value": "71266-105"}])
    );
}

#[test]
fn identity_documents_need_confirmation_and_https() {
    let f = fixture();
    add_identity(&f);
    let no = fill_identity(&f, "https://shop.com/", None, &["cpf"], false);
    assert_eq!(no["result"]["values"], serde_json::json!([]));
    let yes = fill_identity(&f, "https://shop.com/", None, &["cpf"], true);
    assert_eq!(yes["result"]["values"][0]["value"], "123.456.789-00");
    let http = fill_identity(&f, "http://shop.com/", None, &["cpf"], true);
    assert_eq!(http["result"]["values"], serde_json::json!([]));
}

/// Attack: evil.com embeds shop.com's checkout in an iframe to harvest the
/// identity through the user's click.
#[test]
fn identity_denied_to_a_cross_site_frame() {
    let f = fixture();
    add_identity(&f);
    let r = fill_identity(
        &f,
        "https://shop.com/checkout",
        Some("https://evil.com/"),
        &["firstName"],
        false,
    );
    assert_eq!(error_code(&r), Some("denied"));
    let r = call(
        &f,
        serde_json::json!({"type": "find_identity", "url": "https://shop.com/", "topUrl": "https://evil.com/"}),
    );
    assert_eq!(error_code(&r), Some("denied"));
}

#[test]
fn identity_requests_respect_lock_integration_and_absence() {
    let f = fixture();
    let r = call(
        &f,
        serde_json::json!({"type": "find_identity", "url": "https://shop.com/"}),
    );
    assert_eq!(error_code(&r), Some("not_found"), "no identity yet");
    add_identity(&f);
    f.vault
        .lock()
        .unwrap()
        .update_settings(Settings {
            browser_integration: false,
            ..Settings::default()
        })
        .unwrap();
    let r = call(
        &f,
        serde_json::json!({"type": "find_identity", "url": "https://shop.com/"}),
    );
    assert_eq!(error_code(&r), Some("integration_disabled"));
    f.vault.lock().unwrap().lock();
    let r = fill_identity(&f, "https://shop.com/", None, &["firstName"], false);
    assert_eq!(error_code(&r), Some("locked"));
}

#[test]
fn open_identity_opens_it_through_the_hook() {
    let f = fixture();
    let id = add_identity(&f);
    let r = call(
        &f,
        serde_json::json!({"type": "open_identity", "url": "https://shop.com/"}),
    );
    assert_eq!(r["result"], serde_json::json!({"type": "open_identity"}));
    assert_eq!(*f.opened.lock().unwrap(), vec![id]);
}

#[test]
fn fill_identity_shares_the_secret_rate_limit() {
    let f = fixture();
    add_identity(&f);
    for _ in 0..10 {
        fill_identity(&f, "https://shop.com/", None, &["firstName"], false);
    }
    assert_eq!(
        error_code(&fill(&f, f.github, "https://github.com/")),
        Some("rate_limited")
    );
}

/// Attack: a request for a thousand roles.
#[test]
fn fill_identity_with_too_many_roles_is_rejected() {
    let f = fixture();
    add_identity(&f);
    let roles = vec!["city"; 1000];
    let r = fill_identity(&f, "https://shop.com/", None, &roles, false);
    assert!(matches!(
        error_code(&r),
        Some("invalid_input") | Some("malformed") | Some("too_large")
    ));
}

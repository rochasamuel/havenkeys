//! CLAUDE.md §46, as an Android caller would attempt it (spec
//! 2026-10-01-android-app §12.3).

use havenkeys_mobile::*;
use std::sync::Arc;

struct Quiet;
impl VaultEvents for Quiet {
    fn locked(&self, _: String) {}
    fn unlocked(&self) {}
    fn connectivity(&self, _: bool) {}
    fn signed_out(&self) {}
    fn items_changed(&self) {}
    fn removed(&self) {}
}

struct Xor;
impl KeystoreCipher for Xor {
    fn seal(&self, p: Vec<u8>) -> Result<Vec<u8>, CipherError> {
        Ok(p.iter().map(|b| b ^ 1).collect())
    }
    fn open(&self, s: Vec<u8>) -> Result<Vec<u8>, CipherError> {
        Ok(s.iter().map(|b| b ^ 1).collect())
    }
}

const CHROME: &str = "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83";

fn chrome(domain: &str) -> TargetFacts {
    TargetFacts {
        package_name: "com.android.chrome".into(),
        signing_certs: vec![havenkeys_core::app_target::parse_fingerprint(CHROME)
            .unwrap()
            .to_vec()],
        web_domain: Some(domain.into()),
        web_scheme: Some("https".into()),
    }
}

fn setup() -> (Arc<MobileVault>, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let v = MobileVault::new(
        MobileConfig {
            data_dir: dir.path().to_string_lossy().into_owned(),
            own_package: "net.havenkeys.android".into(),
        },
        Arc::new(Quiet),
        Arc::new(Xor),
    )
    .unwrap();
    havenkeys_mobile::testing::seed_with_github_login(&v);
    let id = v
        .list_items()
        .unwrap()
        .into_iter()
        .find(|i| i.title == "GitHub")
        .unwrap()
        .id;
    (v, id, dir)
}

#[test]
fn attack_1_a_page_on_evil_com_gets_nothing_for_github() {
    let (v, id, _d) = setup();
    assert!(v.autofill_matches(chrome("evil.com")).unwrap().is_empty());
    assert!(v.autofill_fill(id, chrome("evil.com")).is_err());
}

#[test]
fn attack_2_an_arbitrary_item_id_needs_a_matching_target() {
    let (v, id, _d) = setup();
    let webview = TargetFacts {
        package_name: "com.evil.app".into(),
        signing_certs: vec![vec![9; 32]],
        web_domain: Some("github.com".into()),
        web_scheme: Some("https".into()),
    };
    assert!(v.autofill_fill(id.clone(), webview).is_err());
    let impostor = TargetFacts {
        package_name: "com.github.android".into(),
        signing_certs: vec![vec![9; 32]],
        web_domain: None,
        web_scheme: None,
    };
    assert!(v.autofill_fill(id, impostor).is_err());
}

#[test]
fn attack_3_a_locked_vault_fills_nothing() {
    let (v, id, _d) = setup();
    v.lock();
    assert!(v.autofill_fill(id.clone(), chrome("github.com")).is_err());
    assert!(v.autofill_totp(id.clone(), chrome("github.com")).is_err());
    assert!(v.reveal(id, "password".into()).is_err());
}

#[test]
fn attack_5_malformed_ids_and_targets_are_refused_without_a_panic() {
    let (v, _id, _d) = setup();
    for id in [
        "",
        "x",
        "00000000-0000-0000-0000-000000000000",
        &"9".repeat(10_000),
    ] {
        assert!(v.autofill_fill(id.into(), chrome("github.com")).is_err());
    }
    let huge = TargetFacts {
        package_name: "a.".repeat(10_000),
        signing_certs: vec![vec![0; 32]; 1000],
        web_domain: Some("x".repeat(100_000)),
        web_scheme: Some("https".into()),
    };
    assert!(v.autofill_matches(huge).is_err());
}

#[test]
fn a_stale_or_tampered_bundle_is_refused() {
    let (v, _id, _d) = setup();
    let mut bundle = v
        .create_unlock_bundle("correct horse battery staple".into(), 1)
        .unwrap();
    v.lock();
    let mut tampered = bundle.clone();
    tampered[5] ^= 1;
    assert!(v.unlock_with_bundle(tampered, 1).is_err());
    bundle.truncate(10);
    assert!(v.unlock_with_bundle(bundle, 1).is_err());
}

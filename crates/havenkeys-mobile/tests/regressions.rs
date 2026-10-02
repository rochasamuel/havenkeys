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

fn code(e: MobileError) -> String {
    match e {
        MobileError::Failed { code, .. } => code,
    }
}

/// Positive control: the fixture really does allow this fill, so the attacks
/// below are refused for their own reason and not because setup is broken.
fn assert_fill_works(v: &MobileVault, id: &str) {
    let matches = v.autofill_matches(chrome("github.com")).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, id);
    let fill = v.autofill_fill(id.into(), chrome("github.com")).unwrap();
    assert_eq!(fill.username.as_deref(), Some("octo"));
    assert!(fill.password.is_some());
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
    assert_fill_works(&v, &id);
    assert!(v.autofill_matches(chrome("evil.com")).unwrap().is_empty());
    assert!(v.autofill_fill(id, chrome("evil.com")).is_err());
}

#[test]
fn attack_2_an_arbitrary_item_id_needs_a_matching_target() {
    let (v, id, _d) = setup();
    assert_fill_works(&v, &id);
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
    assert_fill_works(&v, &id);
    assert!(v.reveal(id.clone(), "password".into()).is_ok());
    v.lock();
    assert!(v.autofill_fill(id.clone(), chrome("github.com")).is_err());
    assert!(v.autofill_totp(id.clone(), chrome("github.com")).is_err());
    assert!(v.reveal(id, "password".into()).is_err());
}

#[test]
fn attack_5_malformed_ids_and_targets_are_refused_without_a_panic() {
    let (v, id, _d) = setup();
    assert_fill_works(&v, &id);
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
    let bundle = v
        .create_unlock_bundle("correct horse battery staple".into(), 1)
        .unwrap();

    // Positive control: the untouched bundle unlocks.
    v.lock();
    let status = v.unlock_with_bundle(bundle.clone(), 1).unwrap();
    assert!(matches!(status.state, LockState::Unlocked));

    // Stale: a bundle made before a reboot (boot count 1) is refused at boot 2.
    v.lock();
    assert_eq!(
        code(v.unlock_with_bundle(bundle.clone(), 2).err().unwrap()),
        "bundle_refused"
    );

    // Layout: version (1 byte), then the vault key. Byte 5 is inside the vault
    // key, so the unlock key no longer opens the vault.
    let mut tampered = bundle.clone();
    tampered[5] ^= 1;
    assert!(v.unlock_with_bundle(tampered, 1).is_err());

    let mut truncated = bundle;
    truncated.truncate(10);
    assert_eq!(
        code(v.unlock_with_bundle(truncated, 1).err().unwrap()),
        "bundle_refused"
    );
}

const GET: &str = r#"{"challenge":"AwMD","rpId":"github.com"}"#;

fn chrome_caller(origin: &str) -> CredentialCaller {
    CredentialCaller {
        package_name: "com.android.chrome".into(),
        signing_certs: chrome("github.com").signing_certs,
        origin: Some(origin.into()),
    }
}

fn app_caller(cert: u8) -> CredentialCaller {
    CredentialCaller {
        package_name: "com.github.android".into(),
        signing_certs: vec![vec![cert; 32]],
        origin: None,
    }
}

fn no_fetches(v: &MobileVault) {
    let mut s = v.settings().unwrap();
    s.asset_links = false;
    v.update_settings(s).unwrap();
}

#[test]
fn attack_an_impostor_app_gets_no_passkey() {
    let (v, _, _dir) = setup();
    no_fetches(&v);
    let (item, cred, _) = havenkeys_mobile::testing::seed_github_passkey(&v);
    havenkeys_mobile::testing::vouch(&v, "github.com", "com.github.android", &[0xab; 32], true);
    // Positive control.
    assert_eq!(
        v.passkey_offers(app_caller(0xab), GET.into())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        code(
            v.passkey_offers(app_caller(0xcd), GET.into())
                .err()
                .unwrap()
        ),
        "denied"
    );
    assert_eq!(
        code(
            v.passkey_sign_in(app_caller(0xcd), GET.into(), None, item, cred)
                .err()
                .unwrap()
        ),
        "denied"
    );
}

#[test]
fn attack_an_app_vouched_only_for_filling_gets_no_passkey() {
    let (v, _, _dir) = setup();
    no_fetches(&v);
    havenkeys_mobile::testing::seed_github_passkey(&v);
    havenkeys_mobile::testing::vouch(&v, "github.com", "com.github.android", &[0xab; 32], false);
    assert_eq!(
        code(
            v.passkey_offers(app_caller(0xab), GET.into())
                .err()
                .unwrap()
        ),
        "denied"
    );
}

#[test]
fn attack_an_app_reporting_a_websites_origin_gets_nothing() {
    let (v, _, _dir) = setup();
    havenkeys_mobile::testing::seed_github_passkey(&v);
    assert_eq!(
        v.passkey_offers(chrome_caller("https://github.com"), GET.into())
            .unwrap()
            .len(),
        1
    );
    let claiming = CredentialCaller {
        origin: Some("https://github.com".into()),
        ..app_caller(0xab)
    };
    assert_eq!(
        code(v.passkey_offers(claiming, GET.into()).err().unwrap()),
        "denied"
    );
}

#[test]
fn attack_an_apps_client_data_hash_is_not_signed() {
    use p256::ecdsa::signature::Verifier;
    use p256::ecdsa::{Signature, VerifyingKey};
    use p256::pkcs8::DecodePublicKey;
    let (v, _, _dir) = setup();
    no_fetches(&v);
    let (item, cred, spki) = havenkeys_mobile::testing::seed_github_passkey(&v);
    havenkeys_mobile::testing::vouch(&v, "github.com", "com.github.android", &[0xab; 32], true);
    let forged = [0x42u8; 32];
    let out: serde_json::Value = serde_json::from_str(
        &v.passkey_sign_in(
            app_caller(0xab),
            GET.into(),
            Some(forged.to_vec()),
            item,
            cred,
        )
        .unwrap(),
    )
    .unwrap();
    let b64 = |k: &str| {
        data_encoding::BASE64URL_NOPAD
            .decode(out["response"][k].as_str().unwrap().as_bytes())
            .unwrap()
    };
    let mut signed = b64("authenticatorData");
    signed.extend_from_slice(&forged);
    let sig = Signature::from_der(&b64("signature")).unwrap();
    assert!(VerifyingKey::from_public_key_der(&spki)
        .unwrap()
        .verify(&signed, &sig)
        .is_err());
}

#[test]
fn attack_a_locked_vault_gives_no_passkey() {
    let (v, _, _dir) = setup();
    let (item, cred, _) = havenkeys_mobile::testing::seed_github_passkey(&v);
    v.lock();
    assert_eq!(
        code(
            v.passkey_offers(chrome_caller("https://github.com"), GET.into())
                .err()
                .unwrap()
        ),
        "locked"
    );
    assert_eq!(
        code(
            v.passkey_sign_in(
                chrome_caller("https://github.com"),
                GET.into(),
                None,
                item,
                cred
            )
            .err()
            .unwrap()
        ),
        "locked"
    );
}

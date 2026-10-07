//! Vault health through the mobile API (spec 2026-10-07-vault-health §8).

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
    fn account_deleted(&self) {}
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

fn code(e: MobileError) -> String {
    match e {
        MobileError::Failed { code, .. } => code,
    }
}

fn setup() -> (Arc<MobileVault>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let v = MobileVault::new(
        MobileConfig {
            data_dir: dir.path().to_string_lossy().into_owned(),
            own_package: "net.havenkeys.android".into(),
            device_name: "Pixel 8".into(),
        },
        Arc::new(Quiet),
        Arc::new(Xor),
    )
    .unwrap();
    havenkeys_mobile::testing::seed_with_github_login(&v);
    (v, dir)
}

#[test]
fn health_lists_the_seeded_login_and_offers_its_help_link() {
    let (v, _d) = setup();
    let report = v.health_report().unwrap();
    let issue = report
        .issues
        .iter()
        .find(|i| !i.dismissed)
        .expect("the github login has an issue");
    // github.com is in the passkey list and the seeded login has no passkey.
    assert!(issue.kinds.contains(&HealthKind::Passkey));
    let url = v
        .health_help_url(issue.item_id.clone(), HealthKind::Passkey)
        .unwrap();
    assert!(url.starts_with("https://"));
    // Dismissing is a write: it needs the server (see round_trip.rs).
    assert_eq!(
        code(
            v.set_health_ignored(issue.item_id.clone(), vec![HealthKind::Passkey])
                .err()
                .unwrap()
        ),
        "offline"
    );
}

#[test]
fn a_locked_vault_returns_locked() {
    let (v, _d) = setup();
    let id = v.list_items().unwrap().remove(0).id;
    v.lock();
    assert_eq!(code(v.health_report().err().unwrap()), "locked");
    assert_eq!(
        code(
            v.set_health_ignored(id.clone(), vec![HealthKind::Weak])
                .err()
                .unwrap()
        ),
        "locked"
    );
    assert_eq!(
        code(v.health_help_url(id, HealthKind::Passkey).err().unwrap()),
        "locked"
    );
}

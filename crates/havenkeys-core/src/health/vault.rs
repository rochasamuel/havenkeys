//! Vault health on the unlocked vault: snapshot, cache, dismissals, help links.

use super::directory::{passkey_sites, rule_host, twofactor_sites};
use super::{HealthCheck, HealthReport, HealthSnapshot, LoginFacts};
use crate::error::{Error, Result};
use crate::model::{ItemDetails, ItemType};
use crate::vault::{dedupe_key_parts, StagedWrite, VaultService};
use uuid::Uuid;

/// A cached report older than this is computed again (spec §5.3).
pub const HEALTH_CACHE_MS: i64 = 10 * 60_000;

impl VaultService {
    pub fn cached_health(&self, now_ms: i64) -> Result<Option<HealthReport>> {
        let s = self.session()?;
        Ok(s.health
            .as_ref()
            .filter(|r| (0..=HEALTH_CACHE_MS).contains(&(now_ms - r.computed_at)))
            .cloned())
    }

    /// Reads every login's password and facts. Call under the vault lock,
    /// then release the lock and run `health::compute` on the result.
    pub fn health_snapshot(&self) -> Result<HealthSnapshot> {
        let s = self.session()?;
        let mut logins = Vec::new();
        for ov in s
            .overviews
            .values()
            .filter(|o| o.item_type == ItemType::Login)
        {
            // An item whose details do not open is skipped, as search skips it.
            let Ok(ItemDetails::Login {
                password,
                password_history,
                health_ignored,
                ..
            }) = self.load_details(&ov.id)
            else {
                continue;
            };
            logins.push(LoginFacts {
                id: ov.id,
                title: ov.title.clone(),
                username: ov.username.clone(),
                hosts: ov.urls.iter().filter_map(|r| rule_host(&r.url)).collect(),
                password,
                last_changed: password_history
                    .first()
                    .map_or(ov.created_at, |p| p.replaced_at),
                has_totp: ov.has_totp,
                has_passkey: ov.has_passkey,
                ignored: health_ignored,
                dedupe: dedupe_key_parts(ov, None),
            });
        }
        Ok(HealthSnapshot {
            logins,
            generation: s.generation,
            epoch: self.epoch(),
        })
    }

    /// Cache `report` if nothing changed since `snapshot` was taken.
    pub fn store_health(&mut self, snapshot: &HealthSnapshot, report: &HealthReport) -> bool {
        if snapshot.epoch != self.epoch() {
            return false;
        }
        match self.session_mut() {
            Ok(s) if s.generation == snapshot.generation => {
                s.health = Some(report.clone());
                true
            }
            _ => false,
        }
    }

    /// Everything under the lock. For tests and callers without a worker
    /// thread; the apps use `HavenClient::health_report`.
    pub fn health_report(&mut self, now_ms: i64) -> Result<HealthReport> {
        if let Some(r) = self.cached_health(now_ms)? {
            return Ok(r);
        }
        let snapshot = self.health_snapshot()?;
        let report = super::compute(&snapshot, now_ms);
        self.store_health(&snapshot, &report);
        Ok(report)
    }

    /// Replace the login's dismissed checks. Keeps `updated_at`: dismissing
    /// is not an edit the user would look for under "recently edited".
    pub fn stage_health_ignored(
        &self,
        id: &Uuid,
        mut checks: Vec<HealthCheck>,
    ) -> Result<StagedWrite> {
        let overview = self.get_item(id)?;
        if overview.item_type != ItemType::Login {
            return Err(Error::Denied);
        }
        checks.sort_unstable();
        checks.dedup();
        let mut details = self.load_details(id)?;
        let ItemDetails::Login { health_ignored, .. } = &mut details else {
            return Err(Error::Corrupted);
        };
        *health_ignored = checks;
        let base = self.store.item_revision(id)?;
        self.stage(overview, Some(&details), base)
    }

    /// The directory's https help page for the login's site.
    pub fn health_help_url(&self, id: &Uuid, check: HealthCheck) -> Result<String> {
        let dir = match check {
            HealthCheck::Passkey => passkey_sites(),
            HealthCheck::TwoFactor => twofactor_sites(),
            _ => return Err(Error::InvalidInput("no help link for this check")),
        };
        let overview = self.get_item(id)?;
        if overview.item_type != ItemType::Login {
            return Err(Error::Denied);
        }
        overview
            .urls
            .iter()
            .filter_map(|r| rule_host(&r.url))
            .find_map(|(_, host)| dir.lookup(&host).and_then(|s| s.help.clone()))
            .ok_or(Error::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use crate::health::{HealthCheck, HEALTH_CACHE_MS};
    use crate::local::tests::unlocked_vault;
    use crate::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use crate::vault::VaultService;
    use crate::{Error, SecretString};
    use uuid::Uuid;

    const NOW: i64 = 1_800_000_000_000;

    fn add(v: &mut VaultService, title: &str, url: &str, pw: &str) -> Uuid {
        let input = ItemInput {
            username: Some("octo".into()),
            urls: vec![UrlRule {
                url: url.into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(SecretString::from(pw)),
            ..ItemInput::blank(ItemType::Login, title.into())
        };
        let staged = v.stage_create(input, NOW).unwrap();
        let id = staged.item_id;
        v.commit_write(staged, 1).unwrap();
        id
    }

    fn dismiss(v: &mut VaultService, id: &Uuid, checks: Vec<HealthCheck>) {
        let staged = v.stage_health_ignored(id, checks).unwrap();
        v.commit_write(staged, 2).unwrap();
    }

    #[test]
    fn the_report_lists_logins_only() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        let note = ItemInput {
            content: SecretUpdate::Set(SecretString::from("password1")),
            ..ItemInput::blank(ItemType::SecureNote, "Note".into())
        };
        let staged = v.stage_create(note, NOW).unwrap();
        v.commit_write(staged, 1).unwrap();
        let r = v.health_report(NOW).unwrap();
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.issues[0].item_id, id);
    }

    #[test]
    fn old_comes_from_the_history_or_the_creation_date() {
        let mut v = unlocked_vault();
        let id = add(
            &mut v,
            "Site",
            "https://unlisted-site.example",
            "q7$Vt!m2Lz#9pWx4Rk@e",
        );
        let later = NOW + crate::health::OLD_AFTER_MS + 1;
        assert!(v.health_report(later).unwrap().issues[0]
            .checks
            .contains(&HealthCheck::Old));
        // Change the password a year later: no longer old.
        let edit = ItemInput {
            username: Some("octo".into()),
            urls: vec![UrlRule {
                url: "https://unlisted-site.example".into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(SecretString::from("Another-q7$Vt!m2Lz#9")),
            ..ItemInput::blank(ItemType::Login, "Site".into())
        };
        let staged = v.stage_update(&id, edit, later).unwrap();
        v.commit_write(staged, 3).unwrap();
        assert!(v.health_report(later).unwrap().issues.is_empty());
    }

    #[test]
    fn dismissing_moves_the_check_and_keeps_updated_at() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        let before = v.get_item(&id).unwrap().updated_at;
        dismiss(&mut v, &id, vec![HealthCheck::Weak, HealthCheck::Weak]);
        assert_eq!(v.get_item(&id).unwrap().updated_at, before);
        let r = v.health_report(NOW).unwrap();
        assert!(r.issues.is_empty());
        assert_eq!(r.dismissed[0].checks, vec![HealthCheck::Weak]);
        // Undo.
        dismiss(&mut v, &id, vec![]);
        assert_eq!(v.health_report(NOW).unwrap().counts.weak, 1);
    }

    #[test]
    fn every_edit_path_keeps_health_ignored() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        dismiss(&mut v, &id, vec![HealthCheck::Weak]);
        // A full editor save (desktop and Android both go through stage_update).
        let edit = ItemInput {
            username: Some("renamed".into()),
            urls: vec![UrlRule {
                url: "https://unlisted-site.example".into(),
                match_type: MatchType::Domain,
            }],
            ..ItemInput::blank(ItemType::Login, "Weak (renamed)".into())
        };
        let staged = v.stage_update(&id, edit, NOW + 1).unwrap();
        v.commit_write(staged, 3).unwrap();
        assert_eq!(
            v.health_report(NOW).unwrap().counts.weak,
            0,
            "kept after an editor save"
        );
        // A password changed from the browser (save prompt → "Update").
        let save = v
            .stage_save_login(
                "https://unlisted-site.example/login",
                None,
                Some("renamed"),
                SecretString::from("password2"),
                crate::vault::SaveTarget::Update(&id),
                NOW + 2,
            )
            .unwrap();
        v.commit_write(save.write, 4).unwrap();
        let r = v.health_report(NOW + 2).unwrap();
        assert_eq!(r.dismissed.len(), 1, "kept after a browser save");
        assert_eq!(r.dismissed[0].checks, vec![HealthCheck::Weak]);
    }

    #[test]
    fn only_logins_can_be_dismissed() {
        let mut v = unlocked_vault();
        let note = ItemInput {
            content: SecretUpdate::Set(SecretString::from("x")),
            ..ItemInput::blank(ItemType::SecureNote, "Note".into())
        };
        let staged = v.stage_create(note, NOW).unwrap();
        let id = staged.item_id;
        v.commit_write(staged, 1).unwrap();
        assert_eq!(
            v.stage_health_ignored(&id, vec![HealthCheck::Weak]).err(),
            Some(Error::Denied)
        );
        assert_eq!(
            v.stage_health_ignored(&Uuid::from_u128(1), vec![]).err(),
            Some(Error::NotFound)
        );
    }

    #[test]
    fn a_locked_vault_answers_locked() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        v.lock();
        assert_eq!(v.health_report(NOW).err(), Some(Error::Locked));
        assert_eq!(v.health_snapshot().err(), Some(Error::Locked));
        assert_eq!(v.cached_health(NOW).err(), Some(Error::Locked));
        assert_eq!(
            v.stage_health_ignored(&id, vec![]).err(),
            Some(Error::Locked)
        );
        assert_eq!(
            v.health_help_url(&id, HealthCheck::Passkey).err(),
            Some(Error::Locked)
        );
    }

    #[test]
    fn the_cache_is_cleared_by_writes_and_expires() {
        let mut v = unlocked_vault();
        add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        v.health_report(NOW).unwrap();
        assert!(v.cached_health(NOW + 1).unwrap().is_some());
        assert!(v
            .cached_health(NOW + HEALTH_CACHE_MS + 1)
            .unwrap()
            .is_none());
        add(&mut v, "Weak 2", "https://other.example", "password1");
        assert!(v.cached_health(NOW + 1).unwrap().is_none());
    }

    #[test]
    fn a_report_from_before_a_change_or_a_lock_is_not_cached() {
        let mut v = unlocked_vault();
        add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        let snap = v.health_snapshot().unwrap();
        let report = crate::health::compute(&snap, NOW);
        add(&mut v, "Weak 2", "https://other.example", "password1");
        assert!(!v.store_health(&snap, &report));
        assert!(v.cached_health(NOW).unwrap().is_none());

        let snap = v.health_snapshot().unwrap();
        let report = crate::health::compute(&snap, NOW);
        v.lock();
        assert!(!v.store_health(&snap, &report));
    }

    #[test]
    fn help_links_come_from_the_directory_for_the_login_s_own_site() {
        let mut v = unlocked_vault();
        let gh = add(
            &mut v,
            "GitHub",
            "https://github.com",
            "q7$Vt!m2Lz#9pWx4Rk@e",
        );
        let other = add(
            &mut v,
            "Other",
            "https://unlisted-site.example",
            "q7$Vt!m2Lz#9pWx4Rk@e",
        );
        let url = v.health_help_url(&gh, HealthCheck::Passkey).unwrap();
        assert!(url.starts_with("https://"));
        assert_eq!(
            v.health_help_url(&other, HealthCheck::Passkey).err(),
            Some(Error::NotFound)
        );
        assert_eq!(
            v.health_help_url(&gh, HealthCheck::Weak).err(),
            Some(Error::InvalidInput("no help link for this check"))
        );
    }
}

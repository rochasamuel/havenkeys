//! Vault health for the phone (spec 2026-10-07-vault-health §8). IDs and
//! check kinds only; the help link is looked up here from the login's own
//! websites, and Kotlin opens it unchanged.

use crate::error::MobileResult;
use crate::items::parse_id;
use crate::vault::MobileVault;
use havenkeys_core::health::{HealthCheck, HealthReport};

#[derive(uniffi::Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HealthKind {
    Weak,
    Reused,
    Old,
    Passkey,
    TwoFactor,
    Insecure,
    Duplicate,
}

impl From<HealthCheck> for HealthKind {
    fn from(c: HealthCheck) -> Self {
        match c {
            HealthCheck::Weak => Self::Weak,
            HealthCheck::Reused => Self::Reused,
            HealthCheck::Old => Self::Old,
            HealthCheck::Passkey => Self::Passkey,
            HealthCheck::TwoFactor => Self::TwoFactor,
            HealthCheck::Insecure => Self::Insecure,
            HealthCheck::Duplicate => Self::Duplicate,
        }
    }
}

impl From<HealthKind> for HealthCheck {
    fn from(k: HealthKind) -> Self {
        match k {
            HealthKind::Weak => Self::Weak,
            HealthKind::Reused => Self::Reused,
            HealthKind::Old => Self::Old,
            HealthKind::Passkey => Self::Passkey,
            HealthKind::TwoFactor => Self::TwoFactor,
            HealthKind::Insecure => Self::Insecure,
            HealthKind::Duplicate => Self::Duplicate,
        }
    }
}

#[derive(uniffi::Record)]
pub struct HealthCountsView {
    pub weak: u32,
    pub reused: u32,
    pub old: u32,
    pub passkey: u32,
    pub two_factor: u32,
    pub insecure: u32,
    pub duplicate: u32,
}

#[derive(uniffi::Record)]
pub struct HealthIssueView {
    pub item_id: String,
    pub kinds: Vec<HealthKind>,
    pub reused_group: Option<u32>,
    pub duplicate_group: Option<u32>,
    pub dismissed: bool,
}

#[derive(uniffi::Record)]
pub struct HealthView {
    pub counts: HealthCountsView,
    pub issues: Vec<HealthIssueView>,
}

fn view(r: HealthReport) -> HealthView {
    let c = r.counts;
    let mut issues: Vec<HealthIssueView> = r
        .issues
        .into_iter()
        .map(|i| HealthIssueView {
            item_id: i.item_id.to_string(),
            kinds: i.checks.into_iter().map(Into::into).collect(),
            reused_group: i.reused_group,
            duplicate_group: i.duplicate_group,
            dismissed: false,
        })
        .collect();
    issues.extend(r.dismissed.into_iter().map(|d| HealthIssueView {
        item_id: d.item_id.to_string(),
        kinds: d.checks.into_iter().map(Into::into).collect(),
        reused_group: None,
        duplicate_group: None,
        dismissed: true,
    }));
    HealthView {
        counts: HealthCountsView {
            weak: c.weak,
            reused: c.reused,
            old: c.old,
            passkey: c.passkey,
            two_factor: c.two_factor,
            insecure: c.insecure,
            duplicate: c.duplicate,
        },
        issues,
    }
}

#[uniffi::export]
impl MobileVault {
    /// Blocking and slow on a large vault: call on `Dispatchers.IO`.
    pub fn health_report(&self) -> MobileResult<HealthView> {
        self.unlocked()?;
        Ok(view(self.client.health_report(havenkeys_client::now_ms())?))
    }

    /// Replaces the login's dismissed checks (send the full list).
    pub fn set_health_ignored(&self, id: String, kinds: Vec<HealthKind>) -> MobileResult<()> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        let client = self.client.clone();
        self.block_on(client.set_health_ignored(id, kinds.into_iter().map(Into::into).collect()))?;
        havenkeys_client::ClientEvents::items_changed(&*self.events);
        Ok(())
    }

    pub fn health_help_url(&self, id: String, kind: HealthKind) -> MobileResult<String> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        Ok(self.client.vault()?.health_help_url(&id, kind.into())?)
    }
}

//! Vault service: lock state machine, key hierarchy and item operations.
//!
//! Every function that touches item content requires an active [`Session`].
//! Locking drops the session, which zeroizes the data key and the decrypted
//! overview cache.

use crate::account::AccountRef;
use crate::crypto::blob::{self, BlobContext, Purpose};
use crate::crypto::kdf::{derive_master_key, KdfParams};
use crate::crypto::keys::{
    derive_auth_key_from_master, derive_data_key, derive_kek_v3, AuthKey, Key256, KEY_LEN,
};
use crate::crypto::secret_key::SecretKey;
use crate::error::{Error, Result};
use crate::import::{ImportReport, ImportedItem};
use crate::model::{
    check_note_content, check_notes, check_password, check_shape, clean_title, clean_urls,
    clean_username, ItemDetails, ItemInput, ItemOverview, ItemType, MatchType, PreviousPassword,
    SecretField, SecretUpdate, Settings, UrlRule, MAX_PASSWORD_HISTORY,
};
use crate::origin::{match_item, site_of, MatchStrength, PageUrl};
use crate::secret::SecretString;
use crate::store::{AccountRecord, HeaderRecord, KeyScheme, Store};
use crate::totp::{self, TotpCode};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use zeroize::Zeroizing;

pub const FORMAT_VERSION: u32 = 1;
pub const MIN_MASTER_PASSWORD_CHARS: usize = 10;
pub const MAX_MASTER_PASSWORD_CHARS: usize = 1024;
pub const MAX_SEARCH_QUERY_CHARS: usize = 256;
/// How long a password fill counts as consent for a site's automatic
/// passkey upgrade (see `passkey::Upgrade`).
pub const UPGRADE_WINDOW_MS: i64 = 5 * 60_000;
/// Recent fills kept per session.
pub const MAX_RECENT_FILLS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultState {
    Locked,
    Unlocking,
    Unlocked,
    Locking,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub state: VaultState,
    pub vault_exists: bool,
    /// Items whose overview failed authentication at unlock (0 normally).
    pub damaged_items: usize,
    /// Items pulled from the server that did not decrypt, recorded for
    /// retry (0 normally). 0 when there is no vault.
    pub unreadable_items: usize,
}

pub(crate) struct Session {
    pub(crate) vault_id: Uuid,
    pub(crate) data_key: Key256,
    pub(crate) overviews: HashMap<Uuid, ItemOverview>,
    pub(crate) settings: Settings,
    pub(crate) damaged_items: usize,
    pub(crate) recent_fills: Vec<RecentFill>,
}

/// A password HavenKeys filled: which login, on which site, when. In memory
/// only, inside the session, so it is gone when the vault locks.
pub(crate) struct RecentFill {
    pub(crate) item_id: Uuid,
    pub(crate) site: String,
    pub(crate) filled_at_ms: i64,
}

impl RecentFill {
    /// Within the window, and not from the future (a clock set back).
    pub(crate) fn is_recent(&self, now_ms: i64) -> bool {
        (0..=UPGRADE_WINDOW_MS).contains(&now_ms.saturating_sub(self.filled_at_ms))
    }
}

/// Snapshot of what is needed to derive the KEK, taken under the vault lock
/// so the expensive Argon2id step can run without holding it.
pub struct UnlockTicket {
    vault_id: Uuid,
    kdf: KdfParams,
    key_scheme: KeyScheme,
    epoch: u64,
}

/// Output of [`UnlockTicket::derive_for_account`].
pub struct UnlockKey(Key256);

impl UnlockTicket {
    /// Does unlocking this vault need the Secret Key? Every scheme this
    /// build understands does; kept as a method (rather than matching on the
    /// variant) because the desktop calls it and a scheme added later must
    /// not silently stop asking.
    pub fn needs_secret_key(&self) -> bool {
        true
    }

    /// Key scheme 3 vaults. Slow (Argon2id).
    pub fn derive_for_account(
        &self,
        password: &SecretString,
        secret_key: &SecretKey,
        account: &AccountRef,
    ) -> Result<UnlockKey> {
        self.derive_session_for_account(password, secret_key, account)
            .map(|(key, _)| key)
    }

    /// The same derivation, also returning the auth key the device needs to
    /// open a server session.
    ///
    /// One Argon2id run yields both: the KEK never leaves the device, and the
    /// auth key exists to be sent to the server. Deriving them separately
    /// would double the cost of every unlock for nothing (docs/crypto.md,
    /// key scheme 3).
    pub fn derive_session_for_account(
        &self,
        password: &SecretString,
        secret_key: &SecretKey,
        account: &AccountRef,
    ) -> Result<(UnlockKey, AuthKey)> {
        if password.is_empty() || password.char_len() > MAX_MASTER_PASSWORD_CHARS {
            return Err(Error::UnlockFailed);
        }
        let master_key = derive_master_key(password, &self.kdf)?;
        let kek = derive_kek_v3(&master_key, secret_key, account)?;
        let auth_key = derive_auth_key_from_master(&master_key, secret_key, account)?;
        Ok((UnlockKey(kek), auth_key))
    }
}

/// A login offered for a page. Deliberately contains no secrets.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: Uuid,
    pub title: String,
    pub username: Option<String>,
    pub has_totp: bool,
    pub strength: MatchStrength,
}

impl std::fmt::Debug for Suggestion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Suggestion")
            .field("id", &self.id)
            .field("strength", &self.strength)
            .finish_non_exhaustive()
    }
}

/// Result of [`VaultService::check_login`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveAction {
    Add,
    Update(Uuid),
    Unchanged,
}

fn normalize_username(u: Option<&str>) -> Option<String> {
    u.map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_lowercase)
}

/// The page a browser request is for: the frame holding the fields and, for
/// an iframe, the tab's top-level page.
struct PageContext {
    frame: PageUrl,
    top: Option<PageUrl>,
}

impl PageContext {
    fn parse(page_url: &str, top_url: Option<&str>) -> Option<Self> {
        let frame = PageUrl::parse(page_url)?;
        // An unparseable top URL denies everything rather than being ignored.
        let top = match top_url {
            Some(t) => Some(PageUrl::parse(t)?),
            None => None,
        };
        Some(Self { frame, top })
    }

    /// How well `item` matches the frame, if it also matches the top page.
    fn matches(&self, item: &ItemOverview) -> Option<MatchStrength> {
        let strength = match_item(item, &self.frame)?;
        if let Some(top) = &self.top {
            match_item(item, top)?;
        }
        Some(strength)
    }

    fn site_title_and_origin(&self) -> Option<(String, String)> {
        self.frame.title_and_origin()
    }
}

/// Exactly what is needed to fill a login form, nothing more.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillCredentials {
    pub username: Option<String>,
    pub password: Option<SecretString>,
}

impl std::fmt::Debug for FillCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FillCredentials(<redacted>)")
    }
}

/// Snapshot for a master-password change (see [`VaultService::begin_rekey`]).
pub struct RekeyTicket {
    header: HeaderRecord,
    epoch: u64,
    /// The account this vault is linked to, read from the local store when
    /// the ticket was taken. Key scheme 3 derives its KEK from it, so it is
    /// never supplied by the caller: a renderer or extension cannot rekey a
    /// vault under an identity of its choosing.
    account: Option<AccountRef>,
}

/// Output of [`RekeyTicket::derive_for_account`].
///
/// Carries both login keys: the server checks `current_auth_key` before it
/// accepts the change, and stores `new_auth_key` in its place.
pub struct Rekeyed {
    kdf: KdfParams,
    wrapped_vault_key: Vec<u8>,
    key_scheme: KeyScheme,
    current_auth_key: AuthKey,
    new_auth_key: AuthKey,
}

impl Rekeyed {
    /// The KDF parameters the new wrap was derived with (not secret).
    pub fn kdf(&self) -> &KdfParams {
        &self.kdf
    }

    /// The login key for the current master password.
    pub fn current_auth_key(&self) -> &AuthKey {
        &self.current_auth_key
    }

    /// The login key for the new master password.
    pub fn new_auth_key(&self) -> &AuthKey {
        &self.new_auth_key
    }
}

impl RekeyTicket {
    /// The account this vault is linked to, if the ticket was taken while it
    /// was. `None` means a hand-edited database whose header claims to be
    /// account-bound but whose account row is missing; callers must refuse
    /// rather than derive a KEK for an identity they invented.
    pub fn account(&self) -> Option<&AccountRef> {
        self.account.as_ref()
    }

    /// The revision the server is asked to move from.
    pub fn base_revision(&self) -> u64 {
        self.header.revision
    }

    /// Master password change for an account-bound vault (key scheme 3).
    /// The Secret Key and the account stay the same; only the vault key is
    /// re-wrapped. Slow (Argon2id).
    pub fn derive_for_account(
        &self,
        current: &SecretString,
        new: &SecretString,
        new_kdf: KdfParams,
        secret_key: &SecretKey,
        account: &AccountRef,
    ) -> Result<Rekeyed> {
        check_new_master_password(new)?;
        let h = &self.header;
        let current_master = derive_master_key(current, &h.kdf)?;
        let current_kek = derive_kek_v3(&current_master, secret_key, account)?;
        let current_auth_key = derive_auth_key_from_master(&current_master, secret_key, account)?;
        let vault_key = unwrap_vault_key(&current_kek, h.vault_id, &h.wrapped_vault_key)?;
        let new_master = derive_master_key(new, &new_kdf)?;
        let new_kek = derive_kek_v3(&new_master, secret_key, account)?;
        let new_auth_key = derive_auth_key_from_master(&new_master, secret_key, account)?;
        Ok(Rekeyed {
            wrapped_vault_key: wrap_vault_key(&new_kek, h.vault_id, &vault_key)?,
            kdf: new_kdf,
            key_scheme: KeyScheme::AccountBound,
            current_auth_key,
            new_auth_key,
        })
    }
}

/// A fully prepared new vault (keys derived, header built). Produced without
/// touching the service so the slow KDF runs outside any lock.
pub struct PreparedVault {
    pub(crate) header: HeaderRecord,
    pub(crate) vault_key: Key256,
}

impl PreparedVault {
    /// The vault's identity. A caller needs it before the vault exists
    /// locally: activation sends it to the server, which stores the vault
    /// under it.
    pub fn vault_id(&self) -> Uuid {
        self.header.vault_id
    }

    /// The KDF parameters a second device will need in order to derive the
    /// same keys. They are not secret — the server serves them to anyone who
    /// asks for this account — but they must be recorded exactly.
    pub fn kdf(&self) -> &KdfParams {
        &self.header.kdf
    }

    /// The header revision this vault starts at.
    pub fn header_revision(&self) -> u64 {
        self.header.revision
    }
}

pub fn check_new_master_password(password: &SecretString) -> Result<()> {
    let n = password.char_len();
    if n < MIN_MASTER_PASSWORD_CHARS {
        return Err(Error::InvalidInput(
            "master password must be at least 10 characters",
        ));
    }
    if n > MAX_MASTER_PASSWORD_CHARS {
        return Err(Error::InvalidInput("master password is too long"));
    }
    Ok(())
}

fn wrap_vault_key(kek: &Key256, vault_id: Uuid, vault_key: &Key256) -> Result<Vec<u8>> {
    blob::seal(
        kek,
        &BlobContext::vault(Purpose::VaultKey, vault_id),
        vault_key.as_bytes(),
    )
}

pub(crate) fn unwrap_vault_key(kek: &Key256, vault_id: Uuid, wrapped: &[u8]) -> Result<Key256> {
    let plain = blob::open(
        kek,
        &BlobContext::vault(Purpose::VaultKey, vault_id),
        wrapped,
    )
    .map_err(|_| Error::UnlockFailed)?;
    let bytes: [u8; KEY_LEN] = plain.as_slice().try_into().map_err(|_| Error::Corrupted)?;
    Ok(Key256::from_bytes(bytes))
}

/// A vault created for an account, with the two things the caller must not
/// lose: the Secret Key (for the Emergency Kit) and the auth key (for the
/// activation request).
pub struct AccountVault {
    pub prepared: PreparedVault,
    pub secret_key: SecretKey,
    pub auth_key: AuthKey,
}

/// Activation: derive keys and build the header for a new account vault
/// (key scheme 3). One Argon2id run yields both the KEK and the auth key.
/// Slow (Argon2id).
pub fn prepare_new_account_vault(
    password: &SecretString,
    account: &AccountRef,
    kdf: KdfParams,
    now_ms: i64,
) -> Result<AccountVault> {
    check_new_master_password(password)?;
    let secret_key = SecretKey::generate()?;
    let master_key = derive_master_key(password, &kdf)?;
    let kek = derive_kek_v3(&master_key, &secret_key, account)?;
    let auth_key = derive_auth_key_from_master(&master_key, &secret_key, account)?;
    let vault_id = Uuid::new_v4();
    let vault_key = Key256::random()?;
    let wrapped_vault_key = wrap_vault_key(&kek, vault_id, &vault_key)?;
    Ok(AccountVault {
        prepared: PreparedVault {
            header: HeaderRecord {
                format_version: FORMAT_VERSION,
                vault_id,
                kdf,
                wrapped_vault_key,
                created_at: now_ms,
                key_scheme: KeyScheme::AccountBound,
                revision: 0,
            },
            vault_key,
        },
        secret_key,
        auth_key,
    })
}

/// The auth key for a later login on a device that already holds the Secret
/// Key. Slow (Argon2id).
pub fn derive_auth_key(
    password: &SecretString,
    secret_key: &SecretKey,
    kdf: &KdfParams,
    account: &AccountRef,
) -> Result<AuthKey> {
    derive_auth_key_from_master(&derive_master_key(password, kdf)?, secret_key, account)
}

pub(crate) fn seal_json<T: Serialize>(
    key: &Key256,
    ctx: &BlobContext,
    value: &T,
) -> Result<Vec<u8>> {
    let plain = Zeroizing::new(serde_json::to_vec(value).map_err(|_| Error::Encryption)?);
    blob::seal(key, ctx, &plain)
}

pub(crate) fn open_json<T: serde::de::DeserializeOwned>(
    key: &Key256,
    ctx: &BlobContext,
    data: &[u8],
) -> Result<T> {
    let plain = blob::open(key, ctx, data)?;
    serde_json::from_slice(&plain).map_err(|_| Error::Corrupted)
}

/// A write the server has not accepted yet. Holds sealed blobs; never logged.
///
/// Produced by [`VaultService::stage_create`], [`VaultService::stage_update`]
/// or [`VaultService::stage_delete`] under the vault lock, sent to the server
/// by a later crate, and recorded locally by
/// [`VaultService::commit_write`] with the revision the server assigned.
pub struct StagedWrite {
    pub item_id: Uuid,
    /// The revision this device last saw, or `None` for a new item.
    pub base_revision: Option<i64>,
    /// `None` for a deletion.
    pub overview: Option<Vec<u8>>,
    pub details: Option<Vec<u8>>,
    epoch: u64,
    plain: Option<ItemOverview>,
}

impl std::fmt::Debug for StagedWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StagedWrite")
            .field("item_id", &self.item_id)
            .field("base_revision", &self.base_revision)
            .finish_non_exhaustive()
    }
}

impl StagedWrite {
    /// Builds a [`StagedWrite`] directly from raw blobs, bypassing the
    /// normal staging path (which seals `overview`/`details` under the
    /// unlocked vault's session and records `plain` for the in-memory
    /// cache). Exists so test code can put arbitrary — including
    /// deliberately malformed — bytes on a test server under a given item
    /// id, without an unlocked vault to stage through. Never used outside
    /// tests: `epoch` is set to `0` and `plain` to `None`, so committing the
    /// result with [`VaultService::commit_write`] would misbehave.
    #[cfg(any(test, feature = "test-util"))]
    pub fn for_test(
        item_id: Uuid,
        base_revision: Option<i64>,
        overview: Vec<u8>,
        details: Vec<u8>,
    ) -> Self {
        StagedWrite {
            item_id,
            base_revision,
            overview: Some(overview),
            details: Some(details),
            epoch: 0,
            plain: None,
        }
    }
}

/// A login from the browser, sealed and ready to send. `item_id` is what the
/// extension is told, whether the login was created or updated.
pub struct StagedSave {
    pub write: StagedWrite,
    pub item_id: Uuid,
}

impl std::fmt::Debug for StagedSave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StagedSave")
            .field("item_id", &self.item_id)
            .finish_non_exhaustive()
    }
}

/// An import sealed and ready to send. Never logged: the writes hold
/// ciphertext, and the counts are all that is safe to show.
pub struct StagedImport {
    pub writes: Vec<StagedWrite>,
    pub report: ImportReport,
}

impl std::fmt::Debug for StagedImport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StagedImport")
            .field("writes", &self.writes.len())
            .field("report", &self.report)
            .finish()
    }
}

pub struct VaultService {
    pub(crate) store: Store,
    state: VaultState,
    session: Option<Session>,
    /// Incremented on every lock. Anything authorized against an older epoch
    /// (an in-flight unlock, a future extension grant) is invalid.
    epoch: u64,
}

impl VaultService {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            state: VaultState::Locked,
            session: None,
            epoch: 0,
        }
    }

    pub fn status(&self) -> Result<VaultStatus> {
        Ok(VaultStatus {
            state: self.state,
            vault_exists: self.store.header()?.is_some(),
            damaged_items: self.session.as_ref().map_or(0, |s| s.damaged_items),
            unreadable_items: self.store.unreadable_count().unwrap_or(0),
        })
    }

    pub fn state(&self) -> VaultState {
        self.state
    }

    pub fn is_unlocked(&self) -> bool {
        self.state == VaultState::Unlocked && self.session.is_some()
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The key scheme of the vault on disk, if there is one.
    pub fn key_scheme(&self) -> Result<Option<KeyScheme>> {
        Ok(self.store.header()?.map(|h| h.key_scheme))
    }

    /// When the vault was created (Unix ms), if there is one.
    pub fn created_at(&self) -> Result<Option<i64>> {
        Ok(self.store.header()?.map(|h| h.created_at))
    }

    /// The vault's ID (not secret; it names the vault to the account's server).
    /// The revision of the header stored locally. A device publishes
    /// `revision + 1` when it changes the wrap, and adopts a higher one the
    /// server serves (after checking its attestation).
    pub fn header_revision(&self) -> Result<Option<u64>> {
        Ok(self.store.header()?.map(|h| h.revision))
    }

    /// The KDF parameters of the header stored locally (not secret). Safe while locked.
    pub fn kdf(&self) -> Result<Option<KdfParams>> {
        Ok(self.store.header()?.map(|h| h.kdf))
    }

    pub fn vault_id(&self) -> Result<Option<Uuid>> {
        Ok(self.store.header()?.map(|h| h.vault_id))
    }

    pub(crate) fn session(&self) -> Result<&Session> {
        match (&self.state, &self.session) {
            (VaultState::Unlocked, Some(s)) => Ok(s),
            _ => Err(Error::Locked),
        }
    }

    pub(crate) fn session_mut(&mut self) -> Result<&mut Session> {
        match (&self.state, &mut self.session) {
            (VaultState::Unlocked, Some(s)) => Ok(s),
            _ => Err(Error::Locked),
        }
    }

    // ------------------------------------------------------------ lifecycle

    /// Activation, or sign-in on a second device: persist an account-bound
    /// vault together with the account it belongs to, and leave it unlocked.
    ///
    /// The account row is written first, so an interruption can leave a
    /// record without a vault (harmless, and overwritten by the retry) but
    /// never a vault without its record. Otherwise a vault could exist whose
    /// rollback floor (`adopt_account_header`) has nothing persisted behind
    /// it.
    pub fn create_account_vault(
        &mut self,
        prepared: PreparedVault,
        account: &AccountRecord,
    ) -> Result<()> {
        if prepared.header.key_scheme != KeyScheme::AccountBound {
            return Err(Error::InvalidInput(
                "this vault does not belong to an account",
            ));
        }
        if self.state != VaultState::Locked {
            return Err(Error::Busy);
        }
        if self.store.header()?.is_some() {
            return Err(Error::VaultExists);
        }
        self.store.set_account(account)?;
        let vault_id = prepared.header.vault_id;
        let data_key = derive_data_key(&prepared.vault_key)?;
        let settings = Settings::default();
        let settings_blob = seal_json(
            &data_key,
            &BlobContext::vault(Purpose::Settings, vault_id),
            &settings,
        )?;
        self.store.insert_header(&prepared.header, &settings_blob)?;
        self.session = Some(Session {
            vault_id,
            data_key,
            overviews: HashMap::new(),
            settings,
            damaged_items: 0,
            recent_fills: Vec::new(),
        });
        self.state = VaultState::Unlocked;
        Ok(())
    }

    /// LOCKED → UNLOCKING. Returns what the caller needs to run the KDF.
    pub fn begin_unlock(&mut self) -> Result<UnlockTicket> {
        match self.state {
            VaultState::Locked => {}
            VaultState::Unlocked => return Err(Error::InvalidInput("vault is already unlocked")),
            VaultState::Unlocking | VaultState::Locking => return Err(Error::Busy),
        }
        let header = self.store.header()?.ok_or(Error::NoVault)?;
        if header.format_version != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        header.kdf.validate()?;
        self.state = VaultState::Unlocking;
        Ok(UnlockTicket {
            vault_id: header.vault_id,
            kdf: header.kdf,
            key_scheme: header.key_scheme,
            epoch: self.epoch,
        })
    }

    /// UNLOCKING → UNLOCKED on success, → LOCKED on any failure.
    pub fn finish_unlock(&mut self, ticket: UnlockTicket, key: Result<UnlockKey>) -> Result<()> {
        if self.state != VaultState::Unlocking || ticket.epoch != self.epoch {
            // A lock happened while the KDF was running; discard the result.
            return Err(Error::Locked);
        }
        match self.complete_unlock(&ticket, key) {
            Ok(session) => {
                self.session = Some(session);
                self.state = VaultState::Unlocked;
                Ok(())
            }
            Err(e) => {
                self.state = VaultState::Locked;
                Err(e)
            }
        }
    }

    fn complete_unlock(&self, ticket: &UnlockTicket, key: Result<UnlockKey>) -> Result<Session> {
        let UnlockKey(kek) = key?;
        let header = self.store.header()?.ok_or(Error::NoVault)?;
        // The header must not have changed between begin and finish.
        if header.vault_id != ticket.vault_id
            || header.kdf != ticket.kdf
            || header.key_scheme != ticket.key_scheme
        {
            return Err(Error::UnlockFailed);
        }
        let vault_key = unwrap_vault_key(&kek, header.vault_id, &header.wrapped_vault_key)?;
        self.open_session(header.vault_id, &vault_key)
    }

    /// Build a session from an unwrapped vault key: derive the data key,
    /// then decrypt settings and item overviews.
    fn open_session(&self, vault_id: Uuid, vault_key: &Key256) -> Result<Session> {
        let data_key = derive_data_key(vault_key)?;

        let settings = match self.store.settings_blob()? {
            Some(b) => open_json::<Settings>(
                &data_key,
                &BlobContext::vault(Purpose::Settings, vault_id),
                &b,
            )
            .ok()
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default(),
            None => Settings::default(),
        };

        let mut overviews = HashMap::new();
        let mut damaged_items = 0;
        for row in self.store.item_overviews()? {
            let parsed = row.and_then(|(id, data)| {
                let ctx = BlobContext::item(Purpose::ItemOverview, vault_id, id);
                let ov: ItemOverview = open_json(&data_key, &ctx, &data)?;
                if ov.id != id {
                    return Err(Error::Corrupted);
                }
                Ok(ov)
            });
            match parsed {
                Ok(ov) => {
                    overviews.insert(ov.id, ov);
                }
                Err(_) => damaged_items += 1,
            }
        }
        Ok(Session {
            vault_id,
            data_key,
            overviews,
            settings,
            damaged_items,
            recent_fills: Vec::new(),
        })
    }

    /// Unlock a key scheme 3 vault. Slow (Argon2id).
    pub fn unlock_for_account(
        &mut self,
        password: &SecretString,
        secret_key: &SecretKey,
        account: &AccountRef,
    ) -> Result<()> {
        let ticket = self.begin_unlock()?;
        let key = ticket.derive_for_account(password, secret_key, account);
        self.finish_unlock(ticket, key)
    }

    /// LOCKED → UNLOCKED with a header newer than the local one, already
    /// verified by `prepare_sign_in`. Refuses another vault, a revision at or
    /// below the floor, or any state but LOCKED.
    ///
    /// This is how a device learns of a master-password change made on
    /// another device: the new password no longer opens the local header, so
    /// the caller signs in against the header the server serves and adopts
    /// it here. Only a [`PreparedVault`] is accepted, and its fields are
    /// crate-private, so the header has necessarily passed
    /// `prepare_sign_in`'s attestation check under the vault key it unwraps.
    /// The rollback floor is the same as `adopt_account_header`'s, so a
    /// hostile server cannot replay an older genuine header.
    ///
    /// `epoch` is [`Self::epoch`] as read when the local unlock failed. The
    /// caller spends seconds on the network and Argon2id in between with the
    /// vault LOCKED; a lock requested meanwhile (screen lock, auto-lock,
    /// window close) advances the epoch, and must win: refused with `Locked`.
    pub fn adopt_and_unlock(&mut self, prepared: PreparedVault, epoch: u64) -> Result<()> {
        if self.state != VaultState::Locked {
            return Err(Error::Busy);
        }
        if epoch != self.epoch {
            return Err(Error::Locked);
        }
        let local = self.store.header()?.ok_or(Error::NoVault)?;
        let floor = self
            .store
            .account()?
            .ok_or(Error::InvalidInput(
                "this vault is not linked to an account",
            ))?
            .max_header_rev
            .max(local.revision as i64);
        let h = &prepared.header;
        if h.vault_id != local.vault_id
            || h.key_scheme != KeyScheme::AccountBound
            || h.format_version != FORMAT_VERSION
        {
            return Err(Error::UnlockFailed);
        }
        if (h.revision as i64) <= floor {
            return Err(Error::UnlockFailed);
        }
        self.store
            .update_key_wrap(&h.kdf, &h.wrapped_vault_key, h.key_scheme, h.revision)?;
        self.store.raise_max_header_rev(h.revision as i64)?;
        let session = self.open_session(h.vault_id, &prepared.vault_key)?;
        self.session = Some(session);
        self.state = VaultState::Unlocked;
        Ok(())
    }

    /// Lock (idempotent). Returns true if the vault was unlocked or unlocking.
    pub fn lock(&mut self) -> bool {
        let was_open = self.state != VaultState::Locked;
        self.state = VaultState::Locking;
        self.session = None; // drops keys + overviews → zeroized
        self.epoch = self.epoch.wrapping_add(1);
        self.state = VaultState::Locked;
        was_open
    }

    /// Lock and swap the underlying store, returning the old one so the
    /// caller can drop it and close the file. Used by "remove this device":
    /// the account's file is set aside and this vault reopens empty.
    pub fn replace_store(&mut self, store: Store) -> Store {
        self.lock();
        std::mem::replace(&mut self.store, store)
    }

    /// Snapshot for a master-password change. The Argon2id derivations then
    /// run in [`RekeyTicket::derive_for_account`] without holding the vault
    /// lock, so a lock request is never delayed by a password change.
    pub fn begin_rekey(&self) -> Result<RekeyTicket> {
        self.session()?;
        let header = self.store.header()?.ok_or(Error::NoVault)?;
        let account = self.store.account()?.map(|a| a.to_ref()).transpose()?;
        Ok(RekeyTicket {
            header,
            epoch: self.epoch,
            account,
        })
    }

    /// The attested header the rekey will produce, at base + 1. Writes
    /// nothing: the desktop sends it to the server first and only commits
    /// once the server has accepted it.
    pub fn encode_rekeyed_header(
        &self,
        ticket: &RekeyTicket,
        rekeyed: &Rekeyed,
    ) -> Result<Vec<u8>> {
        let session = self.session()?;
        if ticket.epoch != self.epoch {
            return Err(Error::Locked);
        }
        let record = HeaderRecord {
            kdf: rekeyed.kdf.clone(),
            wrapped_vault_key: rekeyed.wrapped_vault_key.clone(),
            key_scheme: rekeyed.key_scheme,
            revision: ticket.header.revision.saturating_add(1),
            ..ticket.header.clone()
        };
        crate::sync::encode_header(&session.data_key, &record)
    }

    /// Persist the rekey at the revision the server assigned (must be base +
    /// 1). Refused if the vault was locked in the meantime, the header
    /// changed since the ticket was taken, the account row is missing (every
    /// vault is account-bound; a race against sign-out, or a hand-edited
    /// database, must not persist a rewrap for an identity nothing can
    /// reproduce), or the revision is not the one the ticket's header leads
    /// to.
    pub fn commit_rekey(
        &mut self,
        ticket: RekeyTicket,
        rekeyed: Result<Rekeyed>,
        revision: u64,
    ) -> Result<()> {
        self.session()?;
        if ticket.epoch != self.epoch {
            return Err(Error::Locked);
        }
        let rekeyed = rekeyed?;
        let current = self.store.header()?.ok_or(Error::NoVault)?;
        if current.vault_id != ticket.header.vault_id
            || current.kdf != ticket.header.kdf
            || current.wrapped_vault_key != ticket.header.wrapped_vault_key
            || current.key_scheme != ticket.header.key_scheme
            || current.revision != ticket.header.revision
        {
            return Err(Error::Busy);
        }
        if self.store.account()?.is_none() {
            return Err(Error::InvalidInput(
                "this vault is not linked to an account",
            ));
        }
        if revision != current.revision.saturating_add(1) {
            return Err(Error::Corrupted);
        }
        let new_revision = revision;
        self.store.update_key_wrap(
            &rekeyed.kdf,
            &rekeyed.wrapped_vault_key,
            rekeyed.key_scheme,
            new_revision,
        )?;
        // A local password change raises the rollback floor too, so a
        // hostile server cannot later replay the header this device just
        // replaced.
        self.store.raise_max_header_rev(new_revision as i64)?;
        Ok(())
    }

    /// The account this vault belongs to, if any. Safe while locked.
    pub fn account(&self) -> Result<Option<AccountRecord>> {
        self.store.account()
    }

    /// Re-wrap the vault key under a new master password. Items are
    /// untouched (see docs/crypto.md for what this does and does not protect
    /// against). The account comes from the local store, never from the
    /// caller.
    ///
    /// Local only. The desktop goes through the server first
    /// (`change_credentials`); this remains for tests and tools.
    pub fn change_master_password_for_account(
        &mut self,
        current: &SecretString,
        new: &SecretString,
        new_kdf: KdfParams,
        secret_key: &SecretKey,
    ) -> Result<()> {
        let ticket = self.begin_rekey()?;
        let account = ticket.account().cloned().ok_or(Error::InvalidInput(
            "this vault is not linked to an account",
        ))?;
        let rekeyed = ticket.derive_for_account(current, new, new_kdf, secret_key, &account);
        let revision = ticket.base_revision().saturating_add(1);
        self.commit_rekey(ticket, rekeyed, revision)
    }

    // ------------------------------------------------------------ settings

    pub fn settings(&self) -> Result<Settings> {
        Ok(self.session()?.settings)
    }

    pub fn update_settings(&mut self, settings: Settings) -> Result<()> {
        settings.validate()?;
        let session = self.session()?;
        let blob = seal_json(
            &session.data_key,
            &BlobContext::vault(Purpose::Settings, session.vault_id),
            &settings,
        )?;
        self.store.write_settings(&blob)?;
        self.session_mut()?.settings = settings;
        Ok(())
    }

    // ------------------------------------------------------------ reading

    pub fn list_items(&self) -> Result<Vec<ItemOverview>> {
        let mut items: Vec<ItemOverview> = self.session()?.overviews.values().cloned().collect();
        items.sort_by_cached_key(|i| (i.title.to_lowercase(), i.id));
        Ok(items)
    }

    /// Case-insensitive search over title, username and website hosts.
    /// Secret fields and note bodies are never searched.
    pub fn search(&self, query: &str) -> Result<Vec<ItemOverview>> {
        let session = self.session()?;
        if query.chars().count() > MAX_SEARCH_QUERY_CHARS {
            return Err(Error::InvalidInput("search query too long"));
        }
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.list_items();
        }
        let mut items: Vec<ItemOverview> = session
            .overviews
            .values()
            .filter(|i| {
                i.title.to_lowercase().contains(&q)
                    || i.username
                        .as_deref()
                        .is_some_and(|u| u.to_lowercase().contains(&q))
                    || i.urls.iter().any(|r| {
                        url::Url::parse(&r.url)
                            .ok()
                            .and_then(|u| u.host_str().map(|h| h.contains(&q)))
                            .unwrap_or(false)
                    })
            })
            .cloned()
            .collect();
        items.sort_by_cached_key(|i| (i.title.to_lowercase(), i.id));
        Ok(items)
    }

    pub fn get_item(&self, id: &Uuid) -> Result<ItemOverview> {
        self.session()?
            .overviews
            .get(id)
            .cloned()
            .ok_or(Error::NotFound)
    }

    pub(crate) fn load_details(&self, id: &Uuid) -> Result<ItemDetails> {
        let session = self.session()?;
        let overview = session.overviews.get(id).ok_or(Error::NotFound)?;
        let data = self.store.item_details(id)?.ok_or(Error::NotFound)?;
        let ctx = BlobContext::item(Purpose::ItemDetails, session.vault_id, *id);
        let details: ItemDetails =
            open_json(&session.data_key, &ctx, &data).map_err(|e| match e {
                Error::Corrupted => Error::Corrupted,
                _ => Error::Decryption,
            })?;
        if details.item_type() != overview.item_type {
            return Err(Error::Corrupted);
        }
        Ok(details)
    }

    /// Decrypt and return exactly one secret field.
    pub fn reveal(&self, id: &Uuid, field: SecretField) -> Result<SecretString> {
        let value = match (self.load_details(id)?, field) {
            (ItemDetails::Login { password, .. }, SecretField::Password) => password,
            (ItemDetails::Login { notes, .. }, SecretField::Notes) => notes,
            (ItemDetails::SecureNote { content }, SecretField::Content) => Some(content),
            _ => return Err(Error::InvalidInput("field does not exist on this item")),
        };
        value.ok_or(Error::NotFound)
    }

    /// Current TOTP code. The TOTP secret never leaves the core.
    pub fn totp_code(&self, id: &Uuid, unix_seconds: u64) -> Result<TotpCode> {
        match self.load_details(id)? {
            ItemDetails::Login {
                totp: Some(cfg), ..
            } => totp::generate(&cfg, unix_seconds),
            _ => Err(Error::NotFound),
        }
    }

    // ------------------------------------------------------------ page-bound access
    //
    // Everything a browser extension may ask for goes through these. The page
    // URL is re-checked against the item's own website rules on every call; the
    // caller's claims about which item "belongs" to a page are never trusted.

    /// Logins whose website rules match the page, best match first.
    /// Returns no secrets: only ID, title and username.
    ///
    /// `top_url` is the tab's top-level page when the fields are in an
    /// iframe (`None` for the top frame). A login is offered in a frame only
    /// if its rules match the frame *and* the page embedding it, so a
    /// `github.com` frame embedded in `evil.com` gets nothing.
    pub fn find_matches(&self, page_url: &str, top_url: Option<&str>) -> Result<Vec<Suggestion>> {
        let session = self.session()?;
        let Some(page) = PageContext::parse(page_url, top_url) else {
            return Ok(Vec::new());
        };
        let mut out: Vec<Suggestion> = session
            .overviews
            .values()
            .filter(|o| o.item_type == ItemType::Login)
            .filter_map(|o| {
                page.matches(o).map(|strength| Suggestion {
                    id: o.id,
                    title: o.title.clone(),
                    username: o.username.clone(),
                    has_totp: o.has_totp,
                    strength,
                })
            })
            .collect();
        out.sort_by_cached_key(|s| (s.strength, s.title.to_lowercase(), s.id));
        Ok(out)
    }

    /// The item, if and only if it is a login whose rules match the page
    /// (and the embedding page, for frames).
    fn authorize_for_page(
        &self,
        id: &Uuid,
        page_url: &str,
        top_url: Option<&str>,
    ) -> Result<&ItemOverview> {
        let session = self.session()?;
        let overview = session.overviews.get(id).ok_or(Error::NotFound)?;
        let page = PageContext::parse(page_url, top_url).ok_or(Error::Denied)?;
        if overview.item_type != ItemType::Login || page.matches(overview).is_none() {
            return Err(Error::Denied);
        }
        Ok(overview)
    }

    /// Username and password for filling the page. Denied unless the item's
    /// own website rules match it. A returned password is remembered for
    /// `UPGRADE_WINDOW_MS` as consent for the site's passkey upgrade.
    pub fn fill_for_page(
        &mut self,
        id: &Uuid,
        page_url: &str,
        top_url: Option<&str>,
        now_ms: i64,
    ) -> Result<FillCredentials> {
        let username = self
            .authorize_for_page(id, page_url, top_url)?
            .username
            .clone();
        let password = match self.load_details(id)? {
            ItemDetails::Login { password, .. } => password,
            ItemDetails::SecureNote { .. } => return Err(Error::Denied),
        };
        if password.is_some() {
            self.record_fill(*id, page_url, now_ms)?;
        }
        Ok(FillCredentials { username, password })
    }

    fn record_fill(&mut self, item_id: Uuid, page_url: &str, now_ms: i64) -> Result<()> {
        let Some(site) = PageUrl::parse(page_url).as_ref().and_then(site_of) else {
            return Ok(());
        };
        let fills = &mut self.session_mut()?.recent_fills;
        fills.retain(|f| f.is_recent(now_ms) && !(f.item_id == item_id && f.site == site));
        fills.push(RecentFill {
            item_id,
            site,
            filled_at_ms: now_ms,
        });
        let excess = fills.len().saturating_sub(MAX_RECENT_FILLS);
        fills.drain(..excess);
        Ok(())
    }

    /// Current TOTP code for filling the page. Same origin binding as
    /// [`fill_for_page`](Self::fill_for_page); the TOTP secret never leaves.
    pub fn totp_for_page(
        &self,
        id: &Uuid,
        page_url: &str,
        top_url: Option<&str>,
        unix_seconds: u64,
    ) -> Result<TotpCode> {
        self.authorize_for_page(id, page_url, top_url)?;
        self.totp_code(id, unix_seconds)
    }

    /// What saving a login the user just submitted on the page would do.
    ///
    /// * `Unchanged`: a login for this page already has this username and
    ///   password. Nothing to offer.
    /// * `Update(id)`: a login for this page has this username with a
    ///   different password.
    /// * `Add`: no login for this page has this username.
    ///
    /// Usernames compare case-insensitively after trimming. Passwords are
    /// compared inside the core and never returned.
    pub fn check_login(
        &self,
        page_url: &str,
        top_url: Option<&str>,
        username: Option<&str>,
        password: &SecretString,
    ) -> Result<SaveAction> {
        if password.is_empty() {
            return Err(Error::InvalidInput("password is required"));
        }
        let wanted = normalize_username(username);
        let candidates = self.find_matches(page_url, top_url)?;
        let mut update = None;
        for c in &candidates {
            let same_user = normalize_username(c.username.as_deref()) == wanted;
            // A password-only form (no username captured) is "unchanged" if
            // it matches any login for the page.
            if !same_user && wanted.is_some() {
                continue;
            }
            match self.load_details(&c.id) {
                Ok(ItemDetails::Login {
                    password: Some(saved),
                    ..
                }) if secrets_equal(&saved, password) => return Ok(SaveAction::Unchanged),
                _ => {}
            }
            if same_user && update.is_none() {
                update = Some(c.id);
            }
        }
        Ok(update.map_or(SaveAction::Add, SaveAction::Update))
    }

    /// What saving a login the user just submitted on the page would do —
    /// without doing it.
    ///
    /// Saving is a write, and writes need a server to accept them (spec
    /// 2026-09-20 §8.4). This always refuses with `Error::Offline`, and
    /// touches neither the store nor the overview cache: a device with no
    /// sync client must not fabricate a server revision for a row the
    /// server never numbered. Origin binding is still checked first, so a
    /// save for the wrong site or for an item that does not match the page
    /// is `Denied`, exactly as before; only a save that would otherwise have
    /// succeeded is `Offline`.
    /// Seal the login a browser form just submitted, ready to send.
    ///
    /// Same authorization as any other write from the extension: an update
    /// must name an item that is actually saved for the page it came from,
    /// and a new login is stored for the frame's own site, never the top
    /// page's. Nothing is written here; the caller sends the staged write and
    /// records it with `commit_write`.
    pub fn stage_save_login(
        &self,
        page_url: &str,
        top_url: Option<&str>,
        username: Option<&str>,
        password: SecretString,
        update: Option<&Uuid>,
        now_ms: i64,
    ) -> Result<StagedSave> {
        if password.is_empty() {
            return Err(Error::InvalidInput("password is required"));
        }
        self.session()?;
        if let Some(id) = update {
            let existing = self.authorize_for_page(id, page_url, top_url)?.clone();
            let input = ItemInput {
                item_type: ItemType::Login,
                title: existing.title.clone(),
                username: existing.username.clone(),
                urls: existing.urls.clone(),
                password: SecretUpdate::Set(password),
                totp: SecretUpdate::Keep,
                notes: SecretUpdate::Keep,
                content: SecretUpdate::Keep,
            };
            return Ok(StagedSave {
                item_id: *id,
                write: self.stage_update(id, input, now_ms)?,
            });
        }
        // Saved for the frame the form was in, as a whole-site rule.
        let page = PageContext::parse(page_url, top_url).ok_or(Error::Denied)?;
        let (title, origin) = page.site_title_and_origin().ok_or(Error::Denied)?;
        let input = ItemInput {
            item_type: ItemType::Login,
            title,
            username: username.map(str::to_owned),
            urls: vec![UrlRule {
                url: origin,
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(password),
            totp: SecretUpdate::Keep,
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
        };
        let write = self.stage_create(input, now_ms)?;
        Ok(StagedSave {
            item_id: write.item_id,
            write,
        })
    }

    pub fn password_history(&self, id: &Uuid) -> Result<Vec<i64>> {
        match self.load_details(id)? {
            ItemDetails::Login {
                password_history, ..
            } => Ok(password_history.iter().map(|p| p.replaced_at).collect()),
            ItemDetails::SecureNote { .. } => Ok(Vec::new()),
        }
    }

    /// Decrypt one previous password (index into [`password_history`](Self::password_history)).
    pub fn reveal_previous_password(&self, id: &Uuid, index: usize) -> Result<SecretString> {
        match self.load_details(id)? {
            ItemDetails::Login {
                mut password_history,
                ..
            } if index < password_history.len() => Ok(password_history.swap_remove(index).password),
            _ => Err(Error::NotFound),
        }
    }

    // ------------------------------------------------------------ writing

    /// Seal a new item's blobs under the vault lock. Nothing is written to
    /// disk; the server must accept the write before [`commit_write`](Self::commit_write)
    /// records it.
    pub fn stage_create(&self, input: ItemInput, now_ms: i64) -> Result<StagedWrite> {
        let id = Uuid::new_v4();
        let (overview, details) = build_item(id, input, None, now_ms, now_ms)?;
        self.stage(overview, Some(&details), None)
    }

    /// Seal an updated item's blobs, carrying the revision this device last
    /// saw for it so the server can detect a conflicting edit.
    pub fn stage_update(&self, id: &Uuid, input: ItemInput, now_ms: i64) -> Result<StagedWrite> {
        let existing = self.get_item(id)?;
        if existing.item_type != input.item_type {
            return Err(Error::InvalidInput("item type cannot change"));
        }
        let current = self.load_details(id)?;
        let (overview, details) =
            build_item(*id, input, Some(current), existing.created_at, now_ms)?;
        let base = self.store.item_revision(id)?;
        self.stage(overview, Some(&details), base)
    }

    /// Stage a deletion. Carries no blobs, only the revision this device
    /// last saw, so the server can detect a conflicting edit.
    pub fn stage_delete(&self, id: &Uuid) -> Result<StagedWrite> {
        let session = self.session()?;
        if !session.overviews.contains_key(id) {
            return Err(Error::NotFound);
        }
        Ok(StagedWrite {
            item_id: *id,
            base_revision: self.store.item_revision(id)?,
            overview: None,
            details: None,
            epoch: self.epoch,
            plain: None,
        })
    }

    pub(crate) fn stage(
        &self,
        overview: ItemOverview,
        details: Option<&ItemDetails>,
        base_revision: Option<i64>,
    ) -> Result<StagedWrite> {
        let session = self.session()?;
        let id = overview.id;
        let ov_blob = seal_json(
            &session.data_key,
            &BlobContext::item(Purpose::ItemOverview, session.vault_id, id),
            &overview,
        )?;
        let det_blob = match details {
            Some(d) => Some(seal_json(
                &session.data_key,
                &BlobContext::item(Purpose::ItemDetails, session.vault_id, id),
                d,
            )?),
            None => None,
        };
        Ok(StagedWrite {
            item_id: id,
            base_revision,
            overview: Some(ov_blob),
            details: det_blob,
            epoch: self.epoch,
            plain: Some(overview),
        })
    }

    /// Seal every imported item, skipping the ones already in the vault.
    ///
    /// Nothing is written: the caller sends these to the server in batches
    /// and records each accepted write with
    /// [`commit_write`](Self::commit_write), exactly as a single edit does.
    /// The report's counts describe what *will* be stored if every batch is
    /// accepted; a batch the server refuses lowers them, which is why the
    /// desktop reports what it committed rather than this figure.
    pub fn stage_import(
        &self,
        items: Vec<ImportedItem>,
        mut report: ImportReport,
        now_ms: i64,
    ) -> Result<StagedImport> {
        // Only items already in the vault count as duplicates; repeated
        // entries inside the export itself are imported as they are.
        let existing = self.dedupe_keys()?;
        let mut writes = Vec::new();
        report.logins = 0;
        report.secure_notes = 0;

        for item in items {
            let id = Uuid::new_v4();
            let created = item.created_at.unwrap_or(now_ms);
            let updated = item.updated_at.unwrap_or(created);
            let Ok((overview, details)) = build_item(id, item.input, None, created, updated) else {
                report.failed += 1;
                continue;
            };
            if existing.contains(&dedupe_key(&overview, &details)) {
                report.skipped_duplicates += 1;
                continue;
            }
            let item_type = overview.item_type;
            let staged = self.stage(overview, Some(&details), None)?;
            match item_type {
                ItemType::Login => report.logins += 1,
                ItemType::SecureNote => report.secure_notes += 1,
            }
            writes.push(staged);
        }
        report.imported = writes.len();
        Ok(StagedImport { writes, report })
    }

    /// What is already here, for import de-duplication.
    fn dedupe_keys(&self) -> Result<HashSet<[u8; 32]>> {
        let session = self.session()?;
        let mut keys = HashSet::new();
        for ov in session.overviews.values() {
            // Secure notes need their body; a damaged one simply isn't a duplicate.
            let details = match ov.item_type {
                ItemType::Login => None,
                ItemType::SecureNote => self.load_details(&ov.id).ok(),
            };
            keys.insert(dedupe_key_parts(ov, details.as_ref()));
        }
        Ok(keys)
    }

    /// Record a write the server accepted at `revision`. Returns the stored
    /// overview, or `None` for a deletion.
    ///
    /// Refused if the vault locked since the write was staged: the blobs
    /// were sealed under a session that no longer exists, and recording them
    /// would put the replica ahead of what this device can read back.
    pub fn commit_write(
        &mut self,
        staged: StagedWrite,
        revision: i64,
    ) -> Result<Option<ItemOverview>> {
        self.session()?;
        if staged.epoch != self.epoch {
            return Err(Error::Locked);
        }
        match (staged.overview, staged.details, staged.plain) {
            (Some(ov), Some(det), Some(overview)) => {
                self.store
                    .upsert_item(&staged.item_id, &ov, &det, revision)?;
                self.session_mut()?
                    .overviews
                    .insert(staged.item_id, overview.clone());
                Ok(Some(overview))
            }
            (None, None, None) => {
                self.store.delete_item(&staged.item_id)?;
                self.session_mut()?.overviews.remove(&staged.item_id);
                Ok(None)
            }
            // `overview`/`details` are `pub` (see `StagedWrite`) so a caller
            // can mutate a value returned by `stage_create`/`stage_update`/
            // `stage_delete` into a shape neither constructor produces
            // before calling this. Not reachable through this crate's own
            // constructors alone, but real as long as those fields are
            // public — kept so such a value is refused, not silently
            // misread as a create or a delete.
            _ => Err(Error::InvalidInput("malformed staged write")),
        }
    }
}

fn dedupe_key(overview: &ItemOverview, details: &ItemDetails) -> [u8; 32] {
    dedupe_key_parts(overview, Some(details))
}

/// Digest identifying "the same item" for import de-duplication. Hashed so the
/// set never holds a second plaintext copy of note bodies.
fn dedupe_key_parts(overview: &ItemOverview, details: Option<&ItemDetails>) -> [u8; 32] {
    let mut h = Sha256::new();
    let field = |h: &mut Sha256, s: &str| {
        h.update((s.len() as u64).to_le_bytes());
        h.update(s.as_bytes());
    };
    match overview.item_type {
        ItemType::Login => {
            field(&mut h, "login");
            field(&mut h, &overview.title);
            field(&mut h, overview.username.as_deref().unwrap_or(""));
            let mut urls: Vec<&str> = overview.urls.iter().map(|r| r.url.as_str()).collect();
            urls.sort_unstable();
            for u in urls {
                field(&mut h, u);
            }
        }
        ItemType::SecureNote => {
            field(&mut h, "note");
            field(&mut h, &overview.title);
            if let Some(ItemDetails::SecureNote { content }) = details {
                field(&mut h, content.expose());
            }
        }
    }
    h.finalize().into()
}

/// Compare two secrets without an early exit on the first differing byte.
/// Both sides are hashed first so the comparison always covers 32 bytes.
fn secrets_equal(a: &SecretString, b: &SecretString) -> bool {
    let da: [u8; 32] = Sha256::digest(a.expose().as_bytes()).into();
    let db: [u8; 32] = Sha256::digest(b.expose().as_bytes()).into();
    da.iter()
        .zip(db.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Validate input and merge with existing secrets.
pub(crate) fn build_item(
    id: Uuid,
    input: ItemInput,
    current: Option<ItemDetails>,
    created_at: i64,
    now_ms: i64,
) -> Result<(ItemOverview, ItemDetails)> {
    check_shape(&input)?;
    let title = clean_title(&input.title)?;
    let ItemInput {
        item_type,
        username,
        urls,
        password,
        totp,
        notes,
        content,
        ..
    } = input;

    let details = match item_type {
        ItemType::Login => {
            let (cur_pw, cur_totp, cur_notes, mut history, passkeys) = match current {
                Some(ItemDetails::Login {
                    password,
                    totp,
                    notes,
                    password_history,
                    passkeys,
                }) => (password, totp, notes, password_history, passkeys),
                Some(_) => return Err(Error::Corrupted),
                None => (None, None, None, Vec::new(), Vec::new()),
            };
            let previous = cur_pw.clone();
            let password = password.apply(cur_pw);
            if let Some(p) = &password {
                check_password(p)?;
            }
            // A replaced or cleared password goes to the history.
            if let Some(old) = previous {
                if !password.as_ref().is_some_and(|p| secrets_equal(p, &old)) {
                    history.insert(
                        0,
                        PreviousPassword {
                            password: old,
                            replaced_at: now_ms,
                        },
                    );
                    history.truncate(MAX_PASSWORD_HISTORY);
                }
            }
            let notes = notes.apply(cur_notes);
            if let Some(n) = &notes {
                check_notes(n)?;
            }
            let totp = match totp {
                SecretUpdate::Keep => cur_totp,
                SecretUpdate::Clear => None,
                SecretUpdate::Set(v) if v.expose().trim().is_empty() => None,
                SecretUpdate::Set(v) => Some(totp::parse_totp_input(v.expose())?),
            };
            ItemDetails::Login {
                password,
                totp,
                notes,
                password_history: history,
                passkeys,
            }
        }
        ItemType::SecureNote => {
            let cur = match current {
                Some(ItemDetails::SecureNote { content }) => Some(content),
                Some(_) => return Err(Error::Corrupted),
                None => None,
            };
            let content = content.apply(cur).unwrap_or_default();
            check_note_content(&content)?;
            ItemDetails::SecureNote { content }
        }
    };

    let (has_password, has_totp, has_notes, has_passkey) = match &details {
        ItemDetails::Login {
            password,
            totp,
            notes,
            passkeys,
            ..
        } => (
            password.is_some(),
            totp.is_some(),
            notes.is_some(),
            !passkeys.is_empty(),
        ),
        ItemDetails::SecureNote { .. } => (false, false, false, false),
    };
    let overview = ItemOverview {
        id,
        item_type,
        title,
        username: match item_type {
            ItemType::Login => clean_username(username.as_deref())?,
            ItemType::SecureNote => None,
        },
        urls: match item_type {
            ItemType::Login => clean_urls(&urls)?,
            ItemType::SecureNote => Vec::new(),
        },
        has_password,
        has_totp,
        has_notes,
        has_passkey,
        created_at,
        updated_at: now_ms,
    };
    Ok((overview, details))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::NormalizedEmail;
    use crate::crypto::kdf::test_params;

    const PASSWORD: &str = "correct horse battery staple";

    /// An account-bound vault whose account row is missing (a hand-edited
    /// database; `create_account_vault` makes it unreachable otherwise) must
    /// refuse to rekey rather than re-wrap under a key nothing can
    /// reproduce. The header is inserted directly, bypassing
    /// `create_account_vault`, because that is the only way left to reach
    /// this state.
    #[test]
    fn rekey_is_refused_when_the_account_record_is_missing() {
        let account = AccountRef::new(
            Uuid::from_u128(7),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let made =
            prepare_new_account_vault(&SecretString::from(PASSWORD), &account, test_params(), 0)
                .unwrap();
        let sk = made.secret_key;

        let mut store = Store::open_in_memory().unwrap();
        let data_key = derive_data_key(&made.prepared.vault_key).unwrap();
        let settings_blob = seal_json(
            &data_key,
            &BlobContext::vault(Purpose::Settings, made.prepared.header.vault_id),
            &Settings::default(),
        )
        .unwrap();
        store
            .insert_header(&made.prepared.header, &settings_blob)
            .unwrap();
        let mut vault = VaultService::new(store);
        vault
            .unlock_for_account(&SecretString::from(PASSWORD), &sk, &account)
            .unwrap();

        let err = vault
            .change_master_password_for_account(
                &SecretString::from(PASSWORD),
                &SecretString::from("a much longer new password"),
                test_params(),
                &sk,
            )
            .unwrap_err();
        assert_eq!(err.code(), "invalid_input");
        assert!(
            err.to_string().contains("not linked to an account"),
            "unexpected message: {err}"
        );
    }

    // `derivation_refuses_a_missing_account` and
    // `derivation_refuses_a_missing_secret_key` were deleted with
    // `derive_kek_for`: `UnlockTicket::derive_session_for_account` takes a
    // `&SecretKey` and an `&AccountRef`, not options, so neither can be
    // missing. The guarantee is now in the signature rather than in a test.
}

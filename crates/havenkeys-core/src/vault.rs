//! Vault service: lock state machine, key hierarchy and item operations.
//!
//! Every function that touches item content requires an active [`Session`].
//! Locking drops the session, which zeroizes the data key and the decrypted
//! overview cache.

use crate::account::AccountRef;
use crate::crypto::blob::{self, BlobContext, Purpose};
use crate::crypto::kdf::{derive_master_key, KdfParams};
use crate::crypto::keys::{
    derive_auth_key_from_master, derive_data_key, derive_kek, derive_kek_v3,
    derive_kek_with_secret_key, AuthKey, Key256, KEY_LEN,
};
use crate::crypto::secret_key::SecretKey;
use crate::error::{Error, Result};
use crate::import::{ImportReport, ImportedItem};
use crate::model::{
    check_note_content, check_notes, check_password, check_shape, clean_title, clean_urls,
    clean_username, ItemDetails, ItemInput, ItemOverview, ItemType, MatchType, PreviousPassword,
    SecretField, SecretUpdate, Settings, UrlRule, MAX_PASSWORD_HISTORY,
};
use crate::origin::{match_item, MatchStrength, PageUrl};
use crate::secret::SecretString;
use crate::store::{HeaderRecord, KeyScheme, Store};
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
}

pub(crate) struct Session {
    pub(crate) vault_id: Uuid,
    pub(crate) data_key: Key256,
    pub(crate) overviews: HashMap<Uuid, ItemOverview>,
    pub(crate) settings: Settings,
    pub(crate) damaged_items: usize,
}

/// Snapshot of what is needed to derive the KEK, taken under the vault lock
/// so the expensive Argon2id step can run without holding it.
pub struct UnlockTicket {
    vault_id: Uuid,
    kdf: KdfParams,
    key_scheme: KeyScheme,
    epoch: u64,
}

/// Output of [`UnlockTicket::derive`].
pub struct UnlockKey(Key256);

impl UnlockTicket {
    /// Does unlocking this vault need the Secret Key?
    pub fn needs_secret_key(&self) -> bool {
        matches!(
            self.key_scheme,
            KeyScheme::PasswordAndSecretKey | KeyScheme::AccountBound
        )
    }

    /// Does unlocking this vault need the account (email + account ID)?
    pub fn needs_account(&self) -> bool {
        self.key_scheme == KeyScheme::AccountBound
    }

    /// Password-only vaults. Slow (Argon2id).
    pub fn derive(&self, password: &SecretString) -> Result<UnlockKey> {
        self.derive_with_secret_key(password, None)
    }

    /// Any vault: the Secret Key is required for key scheme 2 and ignored
    /// otherwise. Slow (Argon2id).
    pub fn derive_with_secret_key(
        &self,
        password: &SecretString,
        secret_key: Option<&SecretKey>,
    ) -> Result<UnlockKey> {
        if password.is_empty() || password.char_len() > MAX_MASTER_PASSWORD_CHARS {
            return Err(Error::UnlockFailed);
        }
        Ok(UnlockKey(derive_kek_for(
            self.key_scheme,
            password,
            &self.kdf,
            &self.vault_id,
            secret_key,
            None,
        )?))
    }

    /// Key scheme 3 vaults. Slow (Argon2id).
    pub fn derive_for_account(
        &self,
        password: &SecretString,
        secret_key: &SecretKey,
        account: &AccountRef,
    ) -> Result<UnlockKey> {
        if password.is_empty() || password.char_len() > MAX_MASTER_PASSWORD_CHARS {
            return Err(Error::UnlockFailed);
        }
        Ok(UnlockKey(derive_kek_for(
            self.key_scheme,
            password,
            &self.kdf,
            &self.vault_id,
            Some(secret_key),
            Some(account),
        )?))
    }
}

/// The KEK for a key scheme. A scheme that needs the Secret Key or the
/// account is refused before any expensive work.
pub(crate) fn derive_kek_for(
    scheme: KeyScheme,
    password: &SecretString,
    kdf: &KdfParams,
    vault_id: &Uuid,
    secret_key: Option<&SecretKey>,
    account: Option<&AccountRef>,
) -> Result<Key256> {
    match (scheme, secret_key, account) {
        (KeyScheme::PasswordOnly, _, _) => derive_kek(&derive_master_key(password, kdf)?, vault_id),
        (KeyScheme::PasswordAndSecretKey, Some(sk), _) => {
            derive_kek_with_secret_key(&derive_master_key(password, kdf)?, sk, vault_id)
        }
        (KeyScheme::AccountBound, Some(sk), Some(account)) => {
            derive_kek_v3(&derive_master_key(password, kdf)?, sk, account)
        }
        (KeyScheme::AccountBound, Some(_), None) => Err(Error::InvalidInput(
            "this vault belongs to an account; sign in instead",
        )),
        (KeyScheme::PasswordAndSecretKey | KeyScheme::AccountBound, None, _) => {
            Err(Error::SecretKeyRequired)
        }
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

/// Output of [`RekeyTicket::derive`].
pub struct Rekeyed {
    kdf: KdfParams,
    wrapped_vault_key: Vec<u8>,
    key_scheme: KeyScheme,
}

impl RekeyTicket {
    pub fn key_scheme(&self) -> KeyScheme {
        self.header.key_scheme
    }

    /// Verify `current`, then wrap the same vault key under `new`, keeping
    /// the key scheme. Password-only vaults. Slow.
    pub fn derive(
        &self,
        current: &SecretString,
        new: &SecretString,
        new_kdf: KdfParams,
    ) -> Result<Rekeyed> {
        self.derive_with_secret_key(current, new, new_kdf, None)
    }

    /// Master password change for any vault; the Secret Key (required for
    /// schemes 2 and 3) stays the same. Slow.
    ///
    /// Key scheme 3 is routed to [`RekeyTicket::derive_for_account`] with the
    /// account this ticket carries, so the one call site the desktop has
    /// covers every scheme.
    pub fn derive_with_secret_key(
        &self,
        current: &SecretString,
        new: &SecretString,
        new_kdf: KdfParams,
        secret_key: Option<&SecretKey>,
    ) -> Result<Rekeyed> {
        check_new_master_password(new)?;
        if self.header.key_scheme == KeyScheme::AccountBound {
            let secret_key = secret_key.ok_or(Error::SecretKeyRequired)?;
            let account = self.account.as_ref().ok_or(Error::InvalidInput(
                "this vault is not linked to an account",
            ))?;
            return self.derive_for_account(current, new, new_kdf, secret_key, account);
        }
        let scheme = self.header.key_scheme;
        let vault_key = self.unwrap(current, secret_key)?;
        let h = &self.header;
        let new_kek = derive_kek_for(scheme, new, &new_kdf, &h.vault_id, secret_key, None)?;
        Ok(Rekeyed {
            wrapped_vault_key: wrap_vault_key(&new_kek, h.vault_id, &vault_key)?,
            kdf: new_kdf,
            key_scheme: scheme,
        })
    }

    /// Add a Secret Key to a password-only vault (key scheme 1 → 2). The
    /// master password stays the same; the vault key is re-wrapped. Slow.
    pub fn derive_secret_key_upgrade(
        &self,
        password: &SecretString,
        new_secret_key: &SecretKey,
        new_kdf: KdfParams,
    ) -> Result<Rekeyed> {
        if self.header.key_scheme != KeyScheme::PasswordOnly {
            return Err(Error::InvalidInput("this vault already has a Secret Key"));
        }
        let vault_key = self.unwrap(password, None)?;
        let h = &self.header;
        let kek = derive_kek_with_secret_key(
            &derive_master_key(password, &new_kdf)?,
            new_secret_key,
            &h.vault_id,
        )?;
        Ok(Rekeyed {
            wrapped_vault_key: wrap_vault_key(&kek, h.vault_id, &vault_key)?,
            kdf: new_kdf,
            key_scheme: KeyScheme::PasswordAndSecretKey,
        })
    }

    /// Link a key scheme 2 vault to an account (scheme 2 → 3). The master
    /// password and the Secret Key stay the same; only the vault key is
    /// re-wrapped, so items are untouched. Slow (Argon2id).
    pub fn derive_account_upgrade(
        &self,
        password: &SecretString,
        secret_key: &SecretKey,
        account: &AccountRef,
        new_kdf: KdfParams,
    ) -> Result<Rekeyed> {
        if self.header.key_scheme != KeyScheme::PasswordAndSecretKey {
            return Err(Error::InvalidInput(
                "add a Secret Key before linking this vault to an account",
            ));
        }
        let vault_key = self.unwrap(password, Some(secret_key))?;
        let kek = derive_kek_v3(&derive_master_key(password, &new_kdf)?, secret_key, account)?;
        Ok(Rekeyed {
            wrapped_vault_key: wrap_vault_key(&kek, self.header.vault_id, &vault_key)?,
            kdf: new_kdf,
            key_scheme: KeyScheme::AccountBound,
        })
    }

    /// Master password change for an account-bound vault (key scheme 3).
    /// The Secret Key and the account stay the same; only the vault key is
    /// re-wrapped. Slow (Argon2id).
    ///
    /// `derive_with_secret_key` cannot do this: it derives the KEK through
    /// `unwrap`, which always passes `account: None`, so scheme 3 is refused
    /// before any expensive work runs. This method calls `derive_kek_v3`
    /// directly with the caller's account instead.
    pub fn derive_for_account(
        &self,
        current: &SecretString,
        new: &SecretString,
        new_kdf: KdfParams,
        secret_key: &SecretKey,
        account: &AccountRef,
    ) -> Result<Rekeyed> {
        check_new_master_password(new)?;
        if self.header.key_scheme != KeyScheme::AccountBound {
            return Err(Error::InvalidInput(
                "this vault is not linked to an account",
            ));
        }
        let h = &self.header;
        let current_kek = derive_kek_v3(&derive_master_key(current, &h.kdf)?, secret_key, account)?;
        let vault_key = unwrap_vault_key(&current_kek, h.vault_id, &h.wrapped_vault_key)?;
        let new_kek = derive_kek_v3(&derive_master_key(new, &new_kdf)?, secret_key, account)?;
        Ok(Rekeyed {
            wrapped_vault_key: wrap_vault_key(&new_kek, h.vault_id, &vault_key)?,
            kdf: new_kdf,
            key_scheme: KeyScheme::AccountBound,
        })
    }

    fn unwrap(&self, password: &SecretString, secret_key: Option<&SecretKey>) -> Result<Key256> {
        let h = &self.header;
        let kek = derive_kek_for(
            h.key_scheme,
            password,
            &h.kdf,
            &h.vault_id,
            secret_key,
            None,
        )?;
        unwrap_vault_key(&kek, h.vault_id, &h.wrapped_vault_key)
    }
}

/// A fully prepared new vault (keys derived, header built). Produced without
/// touching the service so the slow KDF runs outside any lock.
pub struct PreparedVault {
    pub(crate) header: HeaderRecord,
    pub(crate) vault_key: Key256,
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

/// Derive keys and build the header for a new password-only vault (key
/// scheme 1). Slow (Argon2id). New vaults in the app use
/// [`prepare_new_vault_with_secret_key`].
pub fn prepare_new_vault(
    password: &SecretString,
    kdf: KdfParams,
    now_ms: i64,
) -> Result<PreparedVault> {
    prepare(password, None, kdf, now_ms)
}

/// Derive keys and build the header for a new vault protected by the master
/// password and a Secret Key (key scheme 2). Slow (Argon2id).
pub fn prepare_new_vault_with_secret_key(
    password: &SecretString,
    secret_key: &SecretKey,
    kdf: KdfParams,
    now_ms: i64,
) -> Result<PreparedVault> {
    prepare(password, Some(secret_key), kdf, now_ms)
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

fn prepare(
    password: &SecretString,
    secret_key: Option<&SecretKey>,
    kdf: KdfParams,
    now_ms: i64,
) -> Result<PreparedVault> {
    check_new_master_password(password)?;
    let vault_id = Uuid::new_v4();
    let key_scheme = if secret_key.is_some() {
        KeyScheme::PasswordAndSecretKey
    } else {
        KeyScheme::PasswordOnly
    };
    let kek = derive_kek_for(key_scheme, password, &kdf, &vault_id, secret_key, None)?;
    let vault_key = Key256::random()?;
    let wrapped_vault_key = wrap_vault_key(&kek, vault_id, &vault_key)?;
    Ok(PreparedVault {
        header: HeaderRecord {
            format_version: FORMAT_VERSION,
            vault_id,
            kdf,
            wrapped_vault_key,
            created_at: now_ms,
            key_scheme,
            revision: 0,
        },
        vault_key,
    })
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

    /// The vault's ID (not secret; it names the vault in the sync folder).
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

    /// Persist a prepared vault and leave it unlocked.
    pub fn create_vault(&mut self, prepared: PreparedVault) -> Result<()> {
        if self.state != VaultState::Locked {
            return Err(Error::Busy);
        }
        if self.store.header()?.is_some() {
            return Err(Error::VaultExists);
        }
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
        let data_key = derive_data_key(&vault_key)?;
        let vault_id = header.vault_id;

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
        })
    }

    /// Convenience for tests and callers that do not need the split flow.
    pub fn unlock(&mut self, password: &SecretString) -> Result<()> {
        let ticket = self.begin_unlock()?;
        let key = ticket.derive(password);
        self.finish_unlock(ticket, key)
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

    /// Lock (idempotent). Returns true if the vault was unlocked or unlocking.
    pub fn lock(&mut self) -> bool {
        let was_open = self.state != VaultState::Locked;
        self.state = VaultState::Locking;
        self.session = None; // drops keys + overviews → zeroized
        self.epoch = self.epoch.wrapping_add(1);
        self.state = VaultState::Locked;
        was_open
    }

    /// Snapshot for a master-password change. The two Argon2id derivations
    /// then run in [`RekeyTicket::derive`] without holding the vault lock, so
    /// a lock request is never delayed by a password change.
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

    /// Persist a re-wrapped vault key. Refused if the vault was locked in the
    /// meantime or the header changed since the ticket was taken.
    pub fn commit_rekey(&mut self, ticket: RekeyTicket, rekeyed: Result<Rekeyed>) -> Result<()> {
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
        {
            return Err(Error::Busy);
        }
        let new_revision = current.revision.saturating_add(1);
        self.store.update_key_wrap(
            &rekeyed.kdf,
            &rekeyed.wrapped_vault_key,
            rekeyed.key_scheme,
            new_revision,
        )?;
        // A local password/scheme change raises the rollback floor too, so a
        // hostile server cannot later replay the header this device just
        // replaced.
        self.store.raise_max_header_rev(new_revision as i64)?;
        Ok(())
    }

    /// Re-wrap the vault key under a new master password. Items are untouched
    /// (see docs/crypto.md for what this does and does not protect against).
    pub fn change_master_password(
        &mut self,
        current: &SecretString,
        new: &SecretString,
        new_kdf: KdfParams,
    ) -> Result<()> {
        let ticket = self.begin_rekey()?;
        let rekeyed = ticket.derive(current, new, new_kdf);
        self.commit_rekey(ticket, rekeyed)
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
    /// own website rules match it.
    pub fn fill_for_page(
        &self,
        id: &Uuid,
        page_url: &str,
        top_url: Option<&str>,
    ) -> Result<FillCredentials> {
        let username = self
            .authorize_for_page(id, page_url, top_url)?
            .username
            .clone();
        let password = match self.load_details(id)? {
            ItemDetails::Login { password, .. } => password,
            ItemDetails::SecureNote { .. } => return Err(Error::Denied),
        };
        Ok(FillCredentials { username, password })
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

    /// Save a login the user submitted on the page, after they confirmed it.
    ///
    /// With `update`, only the password of that login changes (the old one
    /// moves to its password history), and only if the login matches the
    /// page. Without, a new login is created for the page's site. Returns
    /// the item ID.
    pub fn save_login(
        &mut self,
        page_url: &str,
        top_url: Option<&str>,
        username: Option<&str>,
        password: SecretString,
        update: Option<&Uuid>,
        now_ms: i64,
    ) -> Result<Uuid> {
        if password.is_empty() {
            return Err(Error::InvalidInput("password is required"));
        }
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
            return self.update_item(id, input, now_ms).map(|o| o.id);
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
        self.create_item(input, now_ms).map(|o| o.id)
    }

    /// When each previous password of a login was replaced, newest first.
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

    pub fn create_item(&mut self, input: ItemInput, now_ms: i64) -> Result<ItemOverview> {
        self.session()?;
        let id = Uuid::new_v4();
        let (overview, details) = build_item(id, input, None, now_ms, now_ms)?;
        self.persist(overview, &details)
    }

    pub fn update_item(
        &mut self,
        id: &Uuid,
        input: ItemInput,
        now_ms: i64,
    ) -> Result<ItemOverview> {
        let existing = self.get_item(id)?;
        if existing.item_type != input.item_type {
            return Err(Error::InvalidInput("item type cannot change"));
        }
        let current = self.load_details(id)?;
        let (overview, details) =
            build_item(*id, input, Some(current), existing.created_at, now_ms)?;
        self.persist(overview, &details)
    }

    /// Delete an item. A tombstone dated `now_ms` records the deletion so it
    /// reaches other devices through sync.
    pub fn delete_item(&mut self, id: &Uuid, now_ms: i64) -> Result<()> {
        if !self.session()?.overviews.contains_key(id) {
            return Err(Error::NotFound);
        }
        // Disk first: if the delete fails, the item must not vanish from view.
        self.store.delete_item(id, now_ms)?;
        self.session_mut()?.overviews.remove(id);
        Ok(())
    }

    /// Store imported items in one transaction.
    ///
    /// Each item goes through the same validation as UI input. Items that fail
    /// validation are counted in `failed`, not fatal. Items already in the vault
    /// before this import are skipped: logins with the same title, username and
    /// websites, and secure notes with the same title and content. This makes
    /// re-importing the same file harmless without dropping entries the export
    /// itself repeats.
    ///
    /// On return, `logins`/`secure_notes` count what was actually stored
    /// (converted items are included in `secure_notes`).
    pub fn import_items(
        &mut self,
        items: Vec<ImportedItem>,
        mut report: ImportReport,
        now_ms: i64,
    ) -> Result<ImportReport> {
        // Only items already in the vault count as duplicates; repeated
        // entries inside the export itself are imported as they are.
        let existing = self.dedupe_keys()?;
        let session = self.session()?;
        let mut rows = Vec::new();
        let mut added = Vec::new();
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
            let ov_blob = seal_json(
                &session.data_key,
                &BlobContext::item(Purpose::ItemOverview, session.vault_id, id),
                &overview,
            )?;
            let det_blob = seal_json(
                &session.data_key,
                &BlobContext::item(Purpose::ItemDetails, session.vault_id, id),
                &details,
            )?;
            match overview.item_type {
                ItemType::Login => report.logins += 1,
                ItemType::SecureNote => report.secure_notes += 1,
            }
            rows.push((id, ov_blob, det_blob));
            added.push(overview);
        }

        self.store.insert_items(&rows)?;
        report.imported = rows.len();
        let session = self.session_mut()?;
        for ov in added {
            session.overviews.insert(ov.id, ov);
        }
        Ok(report)
    }

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

    fn persist(&mut self, overview: ItemOverview, details: &ItemDetails) -> Result<ItemOverview> {
        let session = self.session()?;
        let id = overview.id;
        let ov_blob = seal_json(
            &session.data_key,
            &BlobContext::item(Purpose::ItemOverview, session.vault_id, id),
            &overview,
        )?;
        let det_blob = seal_json(
            &session.data_key,
            &BlobContext::item(Purpose::ItemDetails, session.vault_id, id),
            details,
        )?;
        self.store.upsert_item(&id, &ov_blob, &det_blob)?;
        self.session_mut()?.overviews.insert(id, overview.clone());
        Ok(overview)
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
fn build_item(
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
            let (cur_pw, cur_totp, cur_notes, mut history) = match current {
                Some(ItemDetails::Login {
                    password,
                    totp,
                    notes,
                    password_history,
                }) => (password, totp, notes, password_history),
                Some(_) => return Err(Error::Corrupted),
                None => (None, None, None, Vec::new()),
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

    let (has_password, has_totp, has_notes) = match &details {
        ItemDetails::Login {
            password,
            totp,
            notes,
            ..
        } => (password.is_some(), totp.is_some(), notes.is_some()),
        ItemDetails::SecureNote { .. } => (false, false, false),
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

    #[test]
    fn account_bound_scheme_refuses_derivation_without_an_account() {
        let sk = SecretKey::generate().unwrap();
        let err = derive_kek_for(
            KeyScheme::AccountBound,
            &SecretString::from(PASSWORD),
            &test_params(),
            &Uuid::nil(),
            Some(&sk),
            None,
        )
        .unwrap_err();
        assert_eq!(err.code(), "invalid_input");
    }

    #[test]
    fn account_bound_scheme_refuses_derivation_without_a_secret_key() {
        let account = AccountRef::new(
            Uuid::from_u128(7),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let err = derive_kek_for(
            KeyScheme::AccountBound,
            &SecretString::from(PASSWORD),
            &test_params(),
            &Uuid::nil(),
            None,
            Some(&account),
        )
        .unwrap_err();
        assert_eq!(err.code(), "secret_key_required");
    }
}

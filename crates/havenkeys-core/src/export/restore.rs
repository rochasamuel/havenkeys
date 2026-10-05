//! Restoring an encrypted backup. A backup is hostile input until proven
//! otherwise: every item is rebuilt through `build_item` with each value
//! passed as new (so every limit applies again), and what `ItemInput`
//! cannot carry — passkeys, password history, app bindings — is checked
//! here before it is put back.

use super::backup::{BackupItem, OpenedBackup};
use crate::app_target::valid_package;
use crate::card::CardInput;
use crate::custom_field::{FieldInput, FieldSection, FieldValue, FieldValueInput, SectionInput};
use crate::error::{Error, Result};
use crate::import::ImportReport;
use crate::model::{
    check_password, AppBinding, ItemDetails, ItemInput, ItemOverview, ItemType, PreviousPassword,
    SecretUpdate, MAX_APP_BINDINGS, MAX_PASSWORD_HISTORY,
};
use crate::passkey::{
    clean_site_name, normalize_rp_id, Passkey, CREDENTIAL_ID_LEN, MAX_PASSKEYS_PER_LOGIN,
    MAX_USER_HANDLE_BYTES,
};
use crate::vault::{build_item, StagedImport, VaultService};

const BAD: Error = Error::InvalidInput("backup item is not valid");

fn set(v: Option<crate::secret::SecretString>) -> SecretUpdate {
    v.map_or(SecretUpdate::Keep, SecretUpdate::Set)
}

fn sections_input(sections: Vec<FieldSection>) -> Vec<SectionInput> {
    sections
        .into_iter()
        .map(|s| SectionInput {
            id: None,
            title: s.title,
            fields: s
                .fields
                .into_iter()
                .map(|f| FieldInput {
                    id: None,
                    label: f.label,
                    value: match f.value {
                        FieldValue::Text(v) => FieldValueInput::Text(v),
                        FieldValue::Url(v) => FieldValueInput::Url(v),
                        FieldValue::Email(v) => FieldValueInput::Email(v),
                        FieldValue::Phone(v) => FieldValueInput::Phone(v),
                        FieldValue::Date(v) => FieldValueInput::Date(v),
                        FieldValue::Address(a) => FieldValueInput::Address(a),
                        FieldValue::Password(p) => FieldValueInput::Password(
                            p.map_or(SecretUpdate::Clear, SecretUpdate::Set),
                        ),
                        FieldValue::Otp(c) => {
                            FieldValueInput::Otp(c.map_or(SecretUpdate::Clear, |c| {
                                SecretUpdate::Set(c.to_otpauth_uri())
                            }))
                        }
                    },
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn check_passkeys(keys: &[Passkey]) -> Result<()> {
    if keys.len() > MAX_PASSKEYS_PER_LOGIN {
        return Err(BAD);
    }
    for k in keys {
        let key_ok = k.private_key.expose().len() == 32
            && p256::ecdsa::SigningKey::from_slice(k.private_key.expose()).is_ok();
        if k.credential_id.0.len() != CREDENTIAL_ID_LEN
            || k.user_handle.0.is_empty()
            || k.user_handle.0.len() > MAX_USER_HANDLE_BYTES
            || normalize_rp_id(&k.rp_id).as_deref() != Some(k.rp_id.as_str())
            || clean_site_name(Some(&k.user_name))?.as_deref() != Some(k.user_name.as_str())
            || clean_site_name(k.display_name.as_deref())? != k.display_name
            || !key_ok
        {
            return Err(BAD);
        }
    }
    Ok(())
}

fn check_history(history: &[PreviousPassword]) -> Result<()> {
    if history.len() > MAX_PASSWORD_HISTORY {
        return Err(BAD);
    }
    history.iter().try_for_each(|h| check_password(&h.password))
}

fn check_bindings(bindings: &[AppBinding]) -> Result<()> {
    let hex_ok = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if bindings.len() > MAX_APP_BINDINGS
        || !bindings
            .iter()
            .all(|b| valid_package(&b.package) && hex_ok(&b.cert_sha256))
    {
        return Err(BAD);
    }
    Ok(())
}

/// Rebuild one backup item as if it were typed in, under `id`.
fn rebuild(id: uuid::Uuid, item: BackupItem) -> Result<(ItemOverview, ItemDetails)> {
    let BackupItem {
        overview: ov,
        details,
    } = item;
    if details.item_type() != ov.item_type {
        return Err(BAD);
    }
    let blank = ItemInput::blank(ov.item_type, ov.title.clone());
    let mut extra = None;
    let input = match details {
        ItemDetails::Login {
            password,
            totp,
            notes,
            password_history,
            passkeys,
            sections,
            app_bindings,
        } => {
            check_history(&password_history)?;
            check_passkeys(&passkeys)?;
            check_bindings(&app_bindings)?;
            extra = Some((password_history, passkeys, app_bindings));
            ItemInput {
                username: ov.username.clone(),
                urls: ov.urls.clone(),
                password: set(password),
                totp: set(totp.map(|c| c.to_otpauth_uri())),
                notes: set(notes),
                auto_sign_in: Some(ov.auto_sign_in),
                sign_in_with: ov.sign_in_with.clone(),
                sections: Some(sections_input(sections)),
                ..blank
            }
        }
        ItemDetails::SecureNote { content } => ItemInput {
            content: SecretUpdate::Set(content),
            ..blank
        },
        ItemDetails::Identity(fields) => ItemInput {
            identity: Some(*fields),
            ..blank
        },
        ItemDetails::Card(f) => {
            let f = *f;
            ItemInput {
                card: Some(CardInput {
                    cardholder_name: f.cardholder_name,
                    brand: f.brand,
                    number: set(f.number),
                    verification_number: set(f.verification_number),
                    expiry: f.expiry,
                    notes: f.notes,
                }),
                ..blank
            }
        }
    };
    let (mut overview, mut details) = build_item(id, input, None, ov.created_at, ov.updated_at)?;
    if let (
        Some((history, keys, bindings)),
        ItemDetails::Login {
            password_history,
            passkeys,
            app_bindings,
            ..
        },
    ) = (extra, &mut details)
    {
        overview.has_passkey = !keys.is_empty();
        *password_history = history;
        *passkeys = keys;
        *app_bindings = bindings;
    }
    Ok((overview, details))
}

impl ImportReport {
    /// A staged restore write the server refused because a live item already
    /// holds its ID (another device wrote it after this one pulled): it moves
    /// from its type's count to `skipped_existing`.
    pub fn restore_found_existing(&mut self, item_type: Option<ItemType>) {
        let count = match item_type {
            Some(ItemType::Login) => Some(&mut self.logins),
            Some(ItemType::SecureNote) => Some(&mut self.secure_notes),
            Some(ItemType::Card) => Some(&mut self.cards),
            Some(ItemType::Identity) => Some(&mut self.identities),
            None => None,
        };
        if let Some(count) = count {
            *count = count.saturating_sub(1);
        }
        self.skipped_existing += 1;
        self.imported = self.imported.saturating_sub(1);
    }
}

impl VaultService {
    /// Seal every backup item not already here (spec 2026-10-05-export §6).
    /// Items keep their IDs; the Identity takes this vault's identity ID.
    /// Nothing is ever overwritten. Items the file held but this version
    /// could not read count as `failed`.
    ///
    /// Every write is staged as new (`base_revision = None`). An item deleted
    /// since the backup was made still has a tombstone on the server, which
    /// refuses such a write; the client resolves that per item when it sends
    /// these (`HavenClient::push_restore`), so pull first: "already here" is
    /// only as fresh as the replica.
    pub fn stage_restore(&self, backup: OpenedBackup, _now_ms: i64) -> Result<StagedImport> {
        let session = self.session()?;
        let identity_id = session.identity_id;
        let mut report = ImportReport {
            failed: backup.unreadable,
            ..ImportReport::default()
        };
        let mut writes = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for item in backup.items {
            let id = match item.overview.item_type {
                ItemType::Identity => identity_id,
                // Only the Identity may live under the identity ID: anything
                // else there would squat the slot the account's Identity
                // is created in.
                _ if item.overview.id == identity_id => {
                    report.failed += 1;
                    continue;
                }
                _ => item.overview.id,
            };
            if session.overviews.contains_key(&id) || !seen.insert(id) {
                report.skipped_existing += 1;
                continue;
            }
            let Ok((overview, details)) = rebuild(id, item) else {
                report.failed += 1;
                continue;
            };
            match overview.item_type {
                ItemType::Login => report.logins += 1,
                ItemType::SecureNote => report.secure_notes += 1,
                ItemType::Card => report.cards += 1,
                ItemType::Identity => report.identities += 1,
            }
            writes.push(self.stage(overview, Some(&details), None)?);
        }
        report.imported = writes.len();
        Ok(StagedImport { writes, report })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::passkey::{B64Url, Passkey, CREDENTIAL_ID_LEN, MAX_PASSKEYS_PER_LOGIN};
    use crate::secret::SecretBytes;

    fn passkey(rp_id: &str, key: Vec<u8>) -> Passkey {
        Passkey {
            credential_id: B64Url(vec![1; CREDENTIAL_ID_LEN]),
            rp_id: rp_id.into(),
            user_handle: B64Url(vec![9]),
            user_name: "octo".into(),
            display_name: None,
            private_key: SecretBytes::new(key),
            created_at: 1,
        }
    }

    #[test]
    fn bad_passkeys_are_refused() {
        let good_key = {
            // A valid P-256 scalar: 31 zero bytes then 1.
            let mut k = vec![0u8; 32];
            k[31] = 1;
            k
        };
        assert!(check_passkeys(&[passkey("github.com", good_key.clone())]).is_ok());
        assert!(check_passkeys(&[passkey("github.com", vec![0; 32])]).is_err()); // zero scalar
        assert!(check_passkeys(&[passkey("github.com", vec![1; 31])]).is_err()); // short
        assert!(check_passkeys(&[passkey("GitHub.com.", good_key.clone())]).is_err()); // not normalized
        let too_many: Vec<_> = (0..=MAX_PASSKEYS_PER_LOGIN)
            .map(|_| passkey("github.com", good_key.clone()))
            .collect();
        assert!(check_passkeys(&too_many).is_err());
    }

    /// A non-Identity item under this vault's identity ID would squat the
    /// slot the account's Identity lives in: it fails, the rest restores.
    #[test]
    fn nothing_but_the_identity_may_take_the_identity_id() {
        use crate::model::{ItemInput, ItemType};
        let vault = crate::local::tests::unlocked_vault();
        let identity_id = vault.session().unwrap().identity_id;
        let note = |id| {
            let input = ItemInput {
                content: SecretUpdate::Set("body".into()),
                ..ItemInput::blank(ItemType::SecureNote, "Note".into())
            };
            let (overview, details) = build_item(id, input, None, 1, 1).unwrap();
            BackupItem { overview, details }
        };
        let other = uuid::Uuid::new_v4();
        let staged = vault
            .stage_restore(
                OpenedBackup {
                    items: vec![note(identity_id), note(other)],
                    unreadable: 0,
                },
                1,
            )
            .unwrap();
        assert_eq!((staged.report.imported, staged.report.failed), (1, 1));
        assert_eq!(staged.writes.len(), 1);
        assert_eq!(staged.writes[0].item_id, other);
    }
}

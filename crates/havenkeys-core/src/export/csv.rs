//! Logins as CSV: `name,url,username,password,note,totp`. Chrome reads the
//! first five; Bitwarden, KeePassXC and HavenKeys also read `totp`. Values
//! are written as they are (the reader is a password manager, not a
//! spreadsheet; the export dialog says not to open it in one).

use super::{count, for_each_item, ExportFormat, ExportSummary, Rendered};
use crate::error::{Error, Result};
use crate::model::ItemDetails;
use crate::secret::SecretString;
use crate::vault::VaultService;
use zeroize::Zeroizing;

pub(super) fn render(vault: &VaultService) -> Result<Rendered> {
    let mut summary = ExportSummary::default();
    let mut out = Zeroizing::new(Vec::new());
    {
        let mut w = ::csv::WriterBuilder::new()
            .terminator(::csv::Terminator::Any(b'\n'))
            .from_writer(&mut *out);
        w.write_record(["name", "url", "username", "password", "note", "totp"])
            .map_err(|_| Error::Encryption)?;
        for_each_item(vault, |ov, d| {
            let Some(d) = d else {
                summary.unreadable += 1;
                return Ok(());
            };
            count(&mut summary, ExportFormat::Csv, ov, &d);
            // A bank account's values end the notes as `label: value` lines.
            let (password, totp, notes, bank) = match d {
                ItemDetails::Login {
                    password,
                    totp,
                    notes,
                    ..
                } => (password, totp.map(|c| c.to_otpauth_uri()), notes, vec![]),
                ItemDetails::BankAccount(f) => {
                    let f = *f;
                    let bank = [
                        ("Bank", f.bank.clone().map(SecretString::new)),
                        ("Agência", f.branch_joined().map(SecretString::new)),
                        ("Conta", f.account_joined()),
                    ];
                    (f.password, None, f.notes, bank.into())
                }
                _ => return Ok(()),
            };
            let mut note = notes.map(|n| n.expose().to_owned()).unwrap_or_default();
            let websites = ov
                .urls
                .iter()
                .skip(1)
                .map(|r| format!("Website: {}", r.url));
            let bank = bank
                .iter()
                .filter_map(|(l, v)| Some(format!("{l}: {}", v.as_ref()?.expose())));
            for line in websites.chain(bank) {
                if !note.is_empty() {
                    note.push('\n');
                }
                note.push_str(&line);
            }
            let note = Zeroizing::new(note);
            w.write_record([
                ov.title.as_str(),
                ov.urls.first().map_or("", |r| r.url.as_str()),
                ov.username.as_deref().unwrap_or(""),
                password.as_ref().map_or("", |p| p.expose()),
                note.as_str(),
                totp.as_ref().map_or("", |t| t.expose()),
            ])
            .map_err(|_| Error::Encryption)
        })?;
        w.flush().map_err(|_| Error::Encryption)?;
    }
    Ok(Rendered {
        bytes: out,
        summary,
    })
}

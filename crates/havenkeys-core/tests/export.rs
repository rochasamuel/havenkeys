//! Export, encrypted backup and restore (spec 2026-10-05-export).

mod common;

use common::*;
use havenkeys_core::export;
use havenkeys_core::Error;

#[test]
fn the_master_password_is_checked_against_this_vault() {
    let (vault, sk) = activated_vault();
    let check = vault.begin_password_check().unwrap();
    check.verify(&secret(PASSWORD), &sk).unwrap();
    assert!(matches!(
        check.verify(&secret("not the password"), &sk),
        Err(Error::UnlockFailed)
    ));
}

#[test]
fn a_locked_vault_gives_no_password_check() {
    let (mut vault, _sk) = activated_vault();
    vault.lock();
    assert!(matches!(vault.begin_password_check(), Err(Error::Locked)));
}

#[test]
fn backup_password_rules() {
    let master = secret(PASSWORD);
    export::check_backup_password(&secret("another long passphrase"), &master).unwrap();
    assert!(matches!(
        export::check_backup_password(&secret("short"), &master),
        Err(Error::InvalidInput(
            "backup password must be at least 10 characters"
        ))
    ));
    assert!(matches!(
        export::check_backup_password(&secret(PASSWORD), &master),
        Err(Error::InvalidInput(
            "backup password must differ from the master password"
        ))
    ));
    let long = "x".repeat(10_000);
    assert!(matches!(
        export::check_backup_password(&secret(&long), &master),
        Err(Error::InvalidInput("backup password is too long"))
    ));
}

use havenkeys_core::export::{ExportFormat, ExportSummary};
use havenkeys_core::vault::VaultService;

fn commit_all(vault: &mut VaultService, inputs: Vec<havenkeys_core::model::ItemInput>) {
    let mut rev = 0;
    for input in inputs {
        rev += 1;
        let w = vault.stage_create(input, NOW).unwrap();
        vault.commit_write(w, rev).unwrap();
    }
}

#[test]
fn summary_counts_what_each_format_leaves_out() {
    let (mut vault, _sk) = activated_vault();
    commit_all(
        &mut vault,
        vec![
            login("GitHub", "octo", "pw1", "https://github.com"),
            login("Bank", "me", "pw2", "https://bank.example"),
            note("Recovery codes", "1111 2222"),
        ],
    );
    let csv = export::summarize(&vault, ExportFormat::Csv).unwrap();
    assert_eq!(
        csv,
        ExportSummary {
            logins: 2,
            secure_notes: 1,
            items_left_out: 1,
            ..Default::default()
        }
    );
    let backup = export::summarize(&vault, ExportFormat::Backup).unwrap();
    assert_eq!(
        (backup.logins, backup.secure_notes, backup.items_left_out),
        (2, 1, 0)
    );
}

#[test]
fn the_backup_payload_holds_every_item() {
    let (mut vault, _sk) = activated_vault();
    commit_all(
        &mut vault,
        vec![login("GitHub", "octo", "pw1", "https://github.com")],
    );
    let rendered = export::render(&vault, ExportFormat::Backup, NOW).unwrap();
    let text = std::str::from_utf8(&rendered.bytes).unwrap();
    assert!(text.contains("\"version\":1"));
    assert!(text.contains("pw1"));
    assert_eq!(rendered.summary.logins, 1);
}

#[test]
fn a_locked_vault_exports_nothing() {
    let (mut vault, _sk) = activated_vault();
    vault.lock();
    assert!(matches!(
        export::render(&vault, ExportFormat::Csv, NOW),
        Err(Error::Locked)
    ));
    assert!(matches!(
        export::summarize(&vault, ExportFormat::Csv),
        Err(Error::Locked)
    ));
}

#[test]
fn default_file_names() {
    assert_eq!(
        export::default_file_name(ExportFormat::Backup, NOW),
        "havenkeys-export-2023-11-14.hkbackup"
    );
    assert_eq!(
        export::default_file_name(ExportFormat::Csv, NOW),
        "havenkeys-export-2023-11-14.csv"
    );
}

/// One login's details no longer open: export skips it, counts it in
/// `unreadable`, and still exports the rest.
#[test]
fn an_unreadable_item_is_counted_not_fatal() {
    use rusqlite::{params, Connection};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let (damaged, sk) = {
        let (mut v, sk) = activated_vault_at(&path);
        let a = v
            .stage_create(login("Fine", "a", "pw1", "https://a.example"), NOW)
            .unwrap();
        v.commit_write(a, 1).unwrap();
        let b = v
            .stage_create(login("Damaged", "b", "pw2", "https://b.example"), NOW + 1)
            .unwrap();
        (v.commit_write(b, 2).unwrap().unwrap().id, sk)
    };
    let c = Connection::open(&path).unwrap();
    let mut blob: Vec<u8> = c
        .query_row(
            "SELECT details FROM items WHERE id = ?1",
            params![damaged.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let last = blob.len() - 1;
    blob[last] ^= 0x01;
    c.execute(
        "UPDATE items SET details = ?1 WHERE id = ?2",
        params![blob, damaged.to_string()],
    )
    .unwrap();
    drop(c);

    let mut v = open_file(&path);
    v.unlock_for_account(&secret(PASSWORD), &sk, &account())
        .unwrap();
    let s = export::summarize(&v, ExportFormat::Csv).unwrap();
    assert_eq!((s.logins, s.unreadable), (1, 1));
    let out = export::render(&v, ExportFormat::Csv, NOW).unwrap();
    // Header + one row (the CSV writer ends every record with "\n").
    assert_eq!(out.bytes.iter().filter(|&&b| b == b'\n').count(), 2);
}

use havenkeys_core::export::backup::{open_backup, seal_backup, HEADER_LEN};

fn backup_of(vault: &VaultService, password: &str) -> Vec<u8> {
    let payload = export::render(vault, ExportFormat::Backup, NOW).unwrap();
    seal_backup(&payload.bytes, &secret(password), &fast_kdf()).unwrap()
}

const BACKUP_PW: &str = "a separate backup passphrase";

#[test]
fn a_backup_opens_with_its_password_only() {
    let (mut vault, _sk) = activated_vault();
    commit_all(
        &mut vault,
        vec![login("GitHub", "octo", "pw1", "https://github.com")],
    );
    let file = backup_of(&vault, BACKUP_PW);
    assert_eq!(&file[..8], b"HKBACKUP");
    // Nothing readable in the file.
    assert!(!file.windows(3).any(|w| w == b"pw1"));
    assert!(!file.windows(6).any(|w| w == b"GitHub"));
    let items = open_backup(&file, &secret(BACKUP_PW)).unwrap().items;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].overview.title, "GitHub");
    assert!(matches!(
        open_backup(&file, &secret("wrong backup passphrase")),
        Err(Error::InvalidInput(
            "wrong backup password, or the file is damaged"
        ))
    ));
}

#[test]
fn every_tampered_byte_is_refused() {
    let (vault, _sk) = activated_vault();
    let file = backup_of(&vault, BACKUP_PW);
    // Version (8), KDF algorithm (9), each KDF parameter, the salt and the blob.
    for pos in [
        8,
        9,
        10,
        14,
        18,
        22,
        37,
        HEADER_LEN,
        HEADER_LEN + 2,
        file.len() - 1,
    ] {
        let mut bad = file.clone();
        bad[pos] ^= 0x01;
        assert!(open_backup(&bad, &secret(BACKUP_PW)).is_err(), "byte {pos}");
    }
}

#[test]
fn malformed_backups_are_refused_with_a_fixed_message() {
    let (vault, _sk) = activated_vault();
    let file = backup_of(&vault, BACKUP_PW);
    let not_ours = Error::InvalidInput("not a HavenKeys backup file");
    for bad in [
        &b""[..],
        b"HKBACKU",
        &file[..HEADER_LEN],
        b"PK\x03\x04 a zip file......................................",
    ] {
        assert_eq!(
            open_backup(bad, &secret(BACKUP_PW))
                .unwrap_err()
                .to_string(),
            not_ours.to_string()
        );
    }
    let mut newer = file.clone();
    newer[8] = 2;
    assert!(matches!(
        open_backup(&newer, &secret(BACKUP_PW)),
        Err(Error::InvalidInput(
            "this backup was made by a newer version of HavenKeys"
        ))
    ));
    // KDF parameters outside the accepted range (memory = 1 GiB + 1 KiB).
    let mut greedy = file.clone();
    greedy[10..14].copy_from_slice(&(1024 * 1024 + 1u32).to_le_bytes());
    assert!(matches!(
        open_backup(&greedy, &secret(BACKUP_PW)),
        Err(Error::InvalidInput("not a HavenKeys backup file"))
    ));
}

#[test]
fn a_payload_with_unknown_fields_is_refused() {
    let payload = br#"{"version":1,"exportedAt":0,"items":[],"extra":true}"#;
    let file = seal_backup(payload, &secret(BACKUP_PW), &fast_kdf()).unwrap();
    // It decrypted, so the password was right: never "wrong password".
    assert!(matches!(
        open_backup(&file, &secret(BACKUP_PW)),
        Err(Error::InvalidInput(
            "this backup can't be read by this version of HavenKeys"
        ))
    ));
    let newer = br#"{"version":2,"exportedAt":0,"items":[]}"#;
    let file = seal_backup(newer, &secret(BACKUP_PW), &fast_kdf()).unwrap();
    assert!(matches!(
        open_backup(&file, &secret(BACKUP_PW)),
        Err(Error::InvalidInput(
            "this backup was made by a newer version of HavenKeys"
        ))
    ));
}

#[test]
fn the_size_limit_is_enforced_on_both_sides() {
    use havenkeys_core::crypto::blob::MIN_BLOB_LEN;
    use havenkeys_core::export::backup::MAX_BACKUP_BYTES;
    let largest = MAX_BACKUP_BYTES as usize - HEADER_LEN - MIN_BLOB_LEN;
    let file = seal_backup(&vec![b'x'; largest], &secret(BACKUP_PW), &fast_kdf()).unwrap();
    assert!(file.len() as u64 <= MAX_BACKUP_BYTES);
    assert!(matches!(
        seal_backup(&vec![b'x'; largest + 1], &secret(BACKUP_PW), &fast_kdf()),
        Err(Error::InvalidInput("backup file is too large"))
    ));
    assert!(matches!(
        open_backup(
            &vec![0u8; MAX_BACKUP_BYTES as usize + 1],
            &secret(BACKUP_PW)
        ),
        Err(Error::InvalidInput("backup file is too large"))
    ));
}

use havenkeys_core::passkey::PasskeyCreate;

const GH: &str = "https://github.com/login";

fn passkey_req(handle: &[u8]) -> PasskeyCreate<'_> {
    PasskeyCreate {
        rp_id: "github.com",
        page_url: GH,
        top_url: None,
        challenge: &[7; 32],
        user_handle: handle,
        user_name: "octo",
        display_name: None,
        item_id: None,
        conditional: false,
    }
}

/// Stage-and-commit every restore write; returns the report.
fn restore_into(
    vault: &mut VaultService,
    file: &[u8],
    base: i64,
) -> havenkeys_core::import::ImportReport {
    let opened = open_backup(file, &secret(BACKUP_PW)).unwrap();
    let staged = vault.stage_restore(opened, NOW).unwrap();
    let mut rev = base;
    for w in staged.writes {
        rev += 1;
        vault.commit_write(w, rev).unwrap();
    }
    staged.report
}

#[test]
fn a_backup_restores_into_a_new_account_with_working_passkeys() {
    let (mut a, _) = activated_vault();
    commit_all(
        &mut a,
        vec![
            note("Recovery", "1111"),
            login("Bank", "me", "pw", "https://bank.example"),
        ],
    );
    let staged = a.stage_passkey_create(passkey_req(&[1]), NOW).unwrap();
    let pk_item = staged.item_id;
    let cred = staged.registration.credential_id.clone();
    a.commit_write(staged.write, 100).unwrap();
    let file = backup_of(&a, BACKUP_PW);

    let (mut b, _) = activated_vault();
    let report = restore_into(&mut b, &file, 0);
    assert_eq!(
        (
            report.imported,
            report.logins,
            report.secure_notes,
            report.failed
        ),
        (3, 2, 1, 0)
    );
    assert!(b.get_item(&pk_item).unwrap().has_passkey);
    b.passkey_assert(&pk_item, &cred, "github.com", GH, None, &[3; 32])
        .unwrap();
}

#[test]
fn restoring_twice_adds_nothing_and_overwrites_nothing() {
    let (mut a, _) = activated_vault();
    commit_all(
        &mut a,
        vec![login("Bank", "me", "pw", "https://bank.example")],
    );
    let file = backup_of(&a, BACKUP_PW);
    let (mut b, _) = activated_vault();
    assert_eq!(restore_into(&mut b, &file, 0).imported, 1);
    // Edited after the first restore: the second must not undo it.
    let id = b.list_items().unwrap()[0].id;
    let w = b
        .stage_update(
            &id,
            login("Bank", "me", "edited", "https://bank.example"),
            NOW + 5,
        )
        .unwrap();
    b.commit_write(w, 5).unwrap();
    let second = restore_into(&mut b, &file, 10);
    assert_eq!((second.imported, second.skipped_existing), (0, 1));
    assert_eq!(b.list_items().unwrap().len(), 1);
    let pw = b
        .reveal(&id, havenkeys_core::model::SecretField::Password)
        .unwrap();
    assert_eq!(pw.expose(), "edited");
}

#[test]
fn the_identity_restores_under_this_vaults_id_or_is_skipped() {
    let (mut a, _) = activated_vault();
    let w = a
        .stage_identity_if_missing("me@example.com", NOW)
        .unwrap()
        .unwrap();
    a.commit_write(w, 1).unwrap();
    let file = backup_of(&a, BACKUP_PW);

    let (mut b, _) = activated_vault();
    let report = restore_into(&mut b, &file, 0);
    assert_eq!(report.identities, 1);
    assert!(b.get_item(&b.identity_item_id().unwrap()).is_ok());
    let again = restore_into(&mut b, &file, 10);
    assert_eq!((again.identities, again.skipped_existing), (0, 1));
}

use havenkeys_core::import::{self, ImportSource};

#[test]
fn bitwarden_json_reimports_and_never_carries_passkeys() {
    let (mut v, _) = activated_vault();
    let mut gh = login("GitHub", "octo", "pw1", "https://github.com");
    gh.totp = havenkeys_core::model::SecretUpdate::Set(secret("JBSWY3DPEHPK3PXP"));
    commit_all(&mut v, vec![gh, note("Recovery", "1111 2222")]);
    let staged = v.stage_passkey_create(passkey_req(&[1]), NOW).unwrap();
    v.commit_write(staged.write, 50).unwrap();

    let out = export::render(&v, ExportFormat::BitwardenJson, NOW).unwrap();
    assert_eq!(out.summary.passkeys_left_out, 1);
    let text = std::str::from_utf8(&out.bytes).unwrap();
    assert!(text.contains("\"fido2Credentials\":[]"));
    assert!(!text.contains("privateKey") && !text.contains("keyValue"));

    let parsed = import::parse(ImportSource::BitwardenJson, &out.bytes).unwrap();
    let (mut fresh, _) = activated_vault();
    let staged = fresh
        .stage_import(parsed.items, parsed.report, NOW)
        .unwrap();
    assert_eq!(staged.report.failed, 0);
    assert_eq!((staged.report.logins, staged.report.secure_notes), (2, 1));
    let mut rev = 0;
    for w in staged.writes {
        rev += 1;
        fresh.commit_write(w, rev).unwrap();
    }
    let github = fresh
        .list_items()
        .unwrap()
        .into_iter()
        .find(|i| i.title == "GitHub")
        .unwrap();
    assert!(github.has_totp);
    assert_eq!(github.username.as_deref(), Some("octo"));
}

/// No plaintext export contains a passkey's private key in any encoding.
#[test]
fn plaintext_exports_never_contain_passkey_key_material() {
    use data_encoding::{BASE64, BASE64URL, BASE64URL_NOPAD, BASE64_NOPAD, HEXLOWER};
    let (mut v, _) = activated_vault();
    let staged = v.stage_passkey_create(passkey_req(&[1]), NOW).unwrap();
    v.commit_write(staged.write, 1).unwrap();
    // The raw key, read from the backup payload (the only place it may be).
    let payload = export::render(&v, ExportFormat::Backup, NOW).unwrap();
    let items: serde_json::Value = serde_json::from_slice(&payload.bytes).unwrap();
    let key_text = items["items"][0]["details"]["passkeys"][0]["privateKey"]
        .as_str()
        .unwrap()
        .to_owned();
    let raw = BASE64URL_NOPAD
        .decode(key_text.as_bytes())
        .or_else(|_| BASE64.decode(key_text.as_bytes()))
        .unwrap();
    let encodings = [
        BASE64.encode(&raw),
        BASE64_NOPAD.encode(&raw),
        BASE64URL.encode(&raw),
        BASE64URL_NOPAD.encode(&raw),
        HEXLOWER.encode(&raw),
    ];
    for format in [ExportFormat::BitwardenJson, ExportFormat::Csv] {
        let out = export::render(&v, format, NOW).unwrap();
        assert!(!out.bytes.windows(raw.len()).any(|w| w == raw.as_slice()));
        let text = String::from_utf8_lossy(&out.bytes);
        for e in &encodings {
            assert!(!text.contains(e.as_str()), "{format:?}");
        }
    }
}

#[test]
fn bitwarden_json_maps_every_item_kind() {
    use havenkeys_core::card::{CardBrand, CardExpiry, CardInput};
    use havenkeys_core::custom_field::{AddressValue, FieldInput, FieldValueInput, SectionInput};
    use havenkeys_core::identity::{CustomField, IdentityFields};
    use havenkeys_core::model::{ItemType, MatchType, SecretUpdate, UrlRule};

    let blank = |item_type, title: &str| {
        let mut i = login(title, "", "", "");
        i.item_type = item_type;
        i.username = None;
        i.urls = vec![];
        i.password = SecretUpdate::Keep;
        i
    };
    let (mut v, _) = activated_vault();
    let field = |id, label: &str, value| FieldInput {
        id,
        label: secret(label),
        value,
    };
    let some = |s: &str| Some(secret(s));
    let rule = |url: &str, match_type| UrlRule {
        url: url.into(),
        match_type,
    };

    // Login with three URL rules and one section of every interesting field.
    let mut gh = login("GitHub", "octo", "old-pw", "https://github.com");
    gh.urls = vec![
        rule("https://github.com", MatchType::Domain),
        rule("https://gist.github.com", MatchType::Origin),
        rule("https://github.com/login", MatchType::Exact),
    ];
    gh.sections = Some(vec![SectionInput {
        id: None,
        title: None,
        fields: vec![
            field(None, "Pet", FieldValueInput::Text(secret("rex"))),
            field(
                None,
                "PIN",
                FieldValueInput::Password(SecretUpdate::Set(secret("4321"))),
            ),
            field(
                None,
                "Old",
                FieldValueInput::Password(SecretUpdate::Set(secret("gone"))),
            ),
            field(
                None,
                "Home",
                FieldValueInput::Address(Box::new(AddressValue {
                    street: some("Rua A"),
                    number: some("5"),
                    ..Default::default()
                })),
            ),
            field(
                None,
                "Token",
                FieldValueInput::Otp(SecretUpdate::Set(secret(
                    "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
                ))),
            ),
        ],
    }]);
    let w = v.stage_create(gh, NOW).unwrap();
    let gh_id = v.commit_write(w, 1).unwrap().unwrap().id;

    // Change the password (old one goes to history) and clear one field.
    let sections = v.login_sections(&gh_id).unwrap();
    let (sid, ids): (_, Vec<_>) = (
        sections[0].id,
        sections[0].fields.iter().map(|f| f.id).collect(),
    );
    let mut edit = login("GitHub", "octo", "new-pw", "https://github.com");
    edit.urls = vec![
        rule("https://github.com", MatchType::Domain),
        rule("https://gist.github.com", MatchType::Origin),
        rule("https://github.com/login", MatchType::Exact),
    ];
    edit.sections = Some(vec![SectionInput {
        id: Some(sid),
        title: None,
        fields: vec![
            field(Some(ids[0]), "Pet", FieldValueInput::Text(secret("rex"))),
            field(
                Some(ids[1]),
                "PIN",
                FieldValueInput::Password(SecretUpdate::Keep),
            ),
            field(
                Some(ids[2]),
                "Old",
                FieldValueInput::Password(SecretUpdate::Clear),
            ),
            field(
                Some(ids[3]),
                "Home",
                FieldValueInput::Address(Box::new(AddressValue {
                    street: some("Rua A"),
                    number: some("5"),
                    ..Default::default()
                })),
            ),
            field(
                Some(ids[4]),
                "Token",
                FieldValueInput::Otp(SecretUpdate::Keep),
            ),
        ],
    }]);
    let w = v.stage_update(&gh_id, edit, NOW + 1).unwrap();
    v.commit_write(w, 2).unwrap();

    // Card.
    let mut card = blank(ItemType::Card, "Visa");
    card.card = Some(CardInput {
        cardholder_name: some("S Rocha"),
        brand: Some(CardBrand::Amex),
        number: SecretUpdate::Set(secret("378282246310005")),
        verification_number: SecretUpdate::Set(secret("1234")),
        expiry: Some(CardExpiry::new(2030, 3).unwrap()),
        notes: None,
    });
    let w = v.stage_create(card, NOW).unwrap();
    v.commit_write(w, 3).unwrap();

    // Identity.
    let w = v
        .stage_identity_if_missing("user@example.com", NOW)
        .unwrap()
        .unwrap();
    let identity_id = w.item_id;
    v.commit_write(w, 4).unwrap();
    let mut ident = blank(ItemType::Identity, "");
    ident.identity = Some(IdentityFields {
        first_name: some("Samuel"),
        last_name: some("Rocha"),
        street: some("Quadra 2"),
        number: some("10"),
        cpf: some("123.456.789-00"),
        home_phone: some("555-0100"),
        custom: vec![CustomField {
            label: "Blood".into(),
            value: "O+".into(),
            hidden: true,
        }],
        ..Default::default()
    });
    let w = v.stage_update(&identity_id, ident, NOW + 1).unwrap();
    v.commit_write(w, 5).unwrap();

    let out = export::render(&v, ExportFormat::BitwardenJson, NOW).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    let items = json["items"].as_array().unwrap();
    let of_type = |t: u64| items.iter().find(|i| i["type"] == t).unwrap();

    let l = of_type(1);
    let uris: Vec<_> = l["login"]["uris"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["match"].clone())
        .collect();
    assert_eq!(uris, [serde_json::Value::Null, 1.into(), 3.into()]);
    assert_eq!(l["login"]["password"], "new-pw");
    assert_eq!(l["passwordHistory"][0]["password"], "old-pw");
    let fields = l["fields"].as_array().unwrap();
    let get = |n: &str| fields.iter().find(|f| f["name"] == n);
    assert_eq!(get("Pet").unwrap()["type"], 0);
    assert_eq!(get("PIN").unwrap()["type"], 1);
    assert_eq!(get("PIN").unwrap()["value"], "4321");
    assert_eq!(get("Home").unwrap()["type"], 0);
    assert_eq!(get("Token").unwrap()["type"], 1);
    assert!(get("Token").unwrap()["value"]
        .as_str()
        .unwrap()
        .starts_with("otpauth://totp/"));
    assert!(get("Old").is_none(), "a cleared password field is skipped");

    let c = of_type(3);
    assert_eq!(c["card"]["brand"], "American Express");
    assert_eq!(c["card"]["number"], "378282246310005");
    assert_eq!(c["card"]["code"], "1234");
    assert_eq!(c["card"]["expMonth"], "3");
    assert_eq!(c["card"]["expYear"], "2030");

    let i = of_type(4);
    assert_eq!(i["identity"]["address1"], "Quadra 2 10");
    assert_eq!(i["identity"]["ssn"], "123.456.789-00");
    assert_eq!(i["identity"]["firstName"], "Samuel");
    let ifields = i["fields"].as_array().unwrap();
    assert!(ifields
        .iter()
        .any(|f| f["name"] == "Home phone" && f["value"] == "555-0100"));
    assert!(ifields
        .iter()
        .any(|f| f["name"] == "Blood" && f["type"] == 1));
}

#[test]
fn csv_quotes_hostile_values_and_reimports_as_chrome() {
    let (mut v, _) = activated_vault();
    let mut tricky = login(
        "Comma, \"quoted\"",
        "=HYPERLINK(\"x\")",
        "p,w\"\n=1",
        "https://a.example",
    );
    tricky.urls.push(havenkeys_core::model::UrlRule {
        url: "https://b.example".into(),
        match_type: havenkeys_core::model::MatchType::Domain,
    });
    tricky.totp = havenkeys_core::model::SecretUpdate::Set(secret("JBSWY3DPEHPK3PXP"));
    commit_all(&mut v, vec![tricky, note("Left out", "x")]);

    let out = export::render(&v, ExportFormat::Csv, NOW).unwrap();
    assert_eq!(out.summary.items_left_out, 1);
    let text = std::str::from_utf8(&out.bytes).unwrap();
    assert!(text.starts_with("name,url,username,password,note,totp\n"));
    assert!(text.contains("otpauth://totp/"));
    assert!(text.contains("Website: https://b.example"));

    let parsed = import::parse(ImportSource::Chrome, &out.bytes).unwrap();
    let (mut fresh, _) = activated_vault();
    let staged = fresh
        .stage_import(parsed.items, parsed.report, NOW)
        .unwrap();
    assert_eq!((staged.report.logins, staged.report.failed), (1, 0));
    fresh
        .commit_write(staged.writes.into_iter().next().unwrap(), 1)
        .unwrap();
    let item = fresh.list_items().unwrap().pop().unwrap();
    // The vault refuses control characters in a title, so the title carries only the comma and quotes.
    assert_eq!(item.title, "Comma, \"quoted\"");
    assert_eq!(item.username.as_deref(), Some("=HYPERLINK(\"x\")"));
    let pw = fresh
        .reveal(&item.id, havenkeys_core::model::SecretField::Password)
        .unwrap();
    assert_eq!(pw.expose(), "p,w\"\n=1");
}

// ------------------------------------------------- restore: round trip, limits

fn seal_json(payload: &serde_json::Value) -> Vec<u8> {
    seal_backup(
        payload.to_string().as_bytes(),
        &secret(BACKUP_PW),
        &fast_kdf(),
    )
    .unwrap()
}

fn payload_of(vault: &VaultService) -> serde_json::Value {
    serde_json::from_slice(
        &export::render(vault, ExportFormat::Backup, NOW)
            .unwrap()
            .bytes,
    )
    .unwrap()
}

/// Custom-field section views with the (regenerated) IDs left out.
fn section_shape(vault: &VaultService, id: &uuid::Uuid) -> serde_json::Value {
    fn strip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                m.remove("id");
                m.values_mut().for_each(strip);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let views = havenkeys_core::custom_field::views(vault.login_sections(id).unwrap());
    let mut v = serde_json::to_value(views).unwrap();
    strip(&mut v);
    v
}

/// Spec §9: every kind of item and value survives backup → restore.
#[test]
fn a_full_vault_round_trips_through_a_backup() {
    use havenkeys_core::card::{CardBrand, CardExpiry, CardField, CardInput};
    use havenkeys_core::custom_field::{
        AddressValue, FieldInput, FieldValue, FieldValueInput, SectionInput,
    };
    use havenkeys_core::identity::IdentityFields;
    use havenkeys_core::model::{ItemType, SecretField, SecretUpdate};

    let (mut a, _) = activated_vault();
    let field = |label: &str, value| FieldInput {
        id: None,
        label: secret(label),
        value,
    };
    let some = |s: &str| Some(secret(s));

    // Login: TOTP, four kinds of custom field, then a password change.
    let mut gh = login("GitHub", "octo", "old-pw", "https://github.com");
    gh.totp = SecretUpdate::Set(secret("JBSWY3DPEHPK3PXP"));
    gh.notes = SecretUpdate::Set(secret("login notes"));
    gh.sections = Some(vec![SectionInput {
        id: None,
        title: some("Extra"),
        fields: vec![
            field("Pet", FieldValueInput::Text(secret("rex"))),
            field(
                "PIN",
                FieldValueInput::Password(SecretUpdate::Set(secret("4321"))),
            ),
            field(
                "Token",
                FieldValueInput::Otp(SecretUpdate::Set(secret(
                    "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
                ))),
            ),
            field(
                "Home",
                FieldValueInput::Address(Box::new(AddressValue {
                    street: some("Rua A"),
                    number: some("5"),
                    ..Default::default()
                })),
            ),
        ],
    }]);
    let w = a.stage_create(gh, NOW).unwrap();
    let gh_id = a.commit_write(w, 1).unwrap().unwrap().id;
    let mut edit = login("GitHub", "octo", "new-pw", "https://github.com");
    edit.totp = SecretUpdate::Keep;
    let w = a.stage_update(&gh_id, edit, NOW + 1).unwrap();
    a.commit_write(w, 2).unwrap();

    // Card, note, identity, passkey.
    let mut card = login("Visa", "", "", "https://x.example");
    card.item_type = ItemType::Card;
    card.username = None;
    card.urls = vec![];
    card.password = SecretUpdate::Keep;
    card.card = Some(CardInput {
        cardholder_name: some("S Rocha"),
        brand: Some(CardBrand::Visa),
        number: SecretUpdate::Set(secret("4111111111111111")),
        verification_number: SecretUpdate::Set(secret("123")),
        expiry: Some(CardExpiry::new(2031, 7).unwrap()),
        notes: some("card notes"),
    });
    let w = a.stage_create(card, NOW).unwrap();
    let card_id = a.commit_write(w, 3).unwrap().unwrap().id;
    let w = a.stage_create(note("Recovery", "1111 2222"), NOW).unwrap();
    let note_id = a.commit_write(w, 4).unwrap().unwrap().id;
    let w = a
        .stage_identity_if_missing("me@example.com", NOW)
        .unwrap()
        .unwrap();
    let ident_a = w.item_id;
    a.commit_write(w, 5).unwrap();
    let mut ident = login("", "", "", "https://x.example");
    ident.item_type = ItemType::Identity;
    ident.username = None;
    ident.urls = vec![];
    ident.password = SecretUpdate::Keep;
    ident.identity = Some(IdentityFields {
        first_name: some("Samuel"),
        last_name: some("Rocha"),
        cpf: some("123.456.789-00"),
        ..Default::default()
    });
    let w = a.stage_update(&ident_a, ident, NOW + 1).unwrap();
    a.commit_write(w, 6).unwrap();
    let staged = a.stage_passkey_create(passkey_req(&[1]), NOW).unwrap();
    let pk_item = staged.item_id;
    let cred = staged.registration.credential_id.clone();
    a.commit_write(staged.write, 7).unwrap();

    let file = backup_of(&a, BACKUP_PW);
    let (mut b, _) = activated_vault();
    let report = restore_into(&mut b, &file, 0);
    assert_eq!(
        (
            report.imported,
            report.logins,
            report.cards,
            report.secure_notes,
            report.identities,
            report.failed
        ),
        (5, 2, 1, 1, 1, 0)
    );

    // Login.
    let reveal = |v: &VaultService, id, f| v.reveal(id, f).unwrap().expose().to_owned();
    assert_eq!(reveal(&b, &gh_id, SecretField::Password), "new-pw");
    assert_eq!(reveal(&b, &gh_id, SecretField::Notes), "login notes");
    assert_eq!(
        b.get_item(&gh_id).unwrap().username.as_deref(),
        Some("octo")
    );
    assert_eq!(
        b.password_history(&gh_id).unwrap(),
        a.password_history(&gh_id).unwrap()
    );
    assert_eq!(
        b.reveal_previous_password(&gh_id, 0).unwrap().expose(),
        "old-pw"
    );
    let t = 1_700_000_000;
    assert_eq!(
        b.totp_code(&gh_id, t).unwrap().code.expose(),
        a.totp_code(&gh_id, t).unwrap().code.expose()
    );
    assert_eq!(section_shape(&b, &gh_id), section_shape(&a, &gh_id));
    let secrets = |v: &VaultService| -> Vec<String> {
        v.login_sections(&gh_id).unwrap()[0]
            .fields
            .iter()
            .filter_map(|f| match &f.value {
                FieldValue::Password(Some(p)) => Some(p.expose().to_owned()),
                FieldValue::Otp(Some(c)) => Some(c.to_otpauth_uri().expose().to_owned()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(secrets(&b), secrets(&a));
    assert_eq!(secrets(&b).len(), 2);

    // Card.
    for f in [
        CardField::CardholderName,
        CardField::Number,
        CardField::VerificationNumber,
        CardField::Expiry,
    ] {
        assert_eq!(
            b.card_value(&card_id, f).unwrap().expose(),
            a.card_value(&card_id, f).unwrap().expose()
        );
    }
    assert_eq!(b.card_fields(card_id).unwrap().brand, Some(CardBrand::Visa));

    // Note.
    assert_eq!(reveal(&b, &note_id, SecretField::Content), "1111 2222");

    // Identity, under b's own identity ID.
    let ident_b = b.identity_item_id().unwrap();
    let got = b.reveal_identity(&ident_b).unwrap();
    assert_eq!(got.first_name.as_ref().unwrap().expose(), "Samuel");
    assert_eq!(got.last_name.as_ref().unwrap().expose(), "Rocha");
    assert_eq!(got.cpf.as_ref().unwrap().expose(), "123.456.789-00");

    // The passkey still signs.
    b.passkey_assert(&pk_item, &cred, "github.com", GH, None, &[3; 32])
        .unwrap();
}

#[test]
fn a_backup_with_too_many_items_is_refused() {
    let items = vec![serde_json::Value::from(0); havenkeys_core::import::MAX_ITEMS + 1];
    let file = seal_json(&serde_json::json!({ "version": 1, "exportedAt": 0, "items": items }));
    assert!(matches!(
        open_backup(&file, &secret(BACKUP_PW)),
        Err(Error::InvalidInput("backup contains too many items"))
    ));
}

#[test]
fn an_unreadable_envelope_is_not_a_wrong_password() {
    for payload in [
        &b"not json"[..],
        br#"{"version":1,"exportedAt":0}"#,
        br#"{"version":0}"#,
    ] {
        let file = seal_backup(payload, &secret(BACKUP_PW), &fast_kdf()).unwrap();
        assert!(matches!(
            open_backup(&file, &secret(BACKUP_PW)),
            Err(Error::InvalidInput(
                "this backup can't be read by this version of HavenKeys"
            ))
        ));
    }
}

/// One item this version cannot read (an unknown type) fails alone.
#[test]
fn an_unknown_item_type_fails_alone() {
    let (mut a, _) = activated_vault();
    commit_all(
        &mut a,
        vec![login("Bank", "me", "pw", "https://bank.example")],
    );
    let mut payload = payload_of(&a);
    let mut alien = payload["items"][0].clone();
    alien["overview"]["id"] = uuid::Uuid::new_v4().to_string().into();
    alien["overview"]["itemType"] = "ssh_key".into();
    alien["details"]["type"] = "ssh_key".into();
    payload["items"].as_array_mut().unwrap().push(alien);

    let opened = open_backup(&seal_json(&payload), &secret(BACKUP_PW)).unwrap();
    assert_eq!((opened.items.len(), opened.unreadable), (1, 1));
    let (mut b, _) = activated_vault();
    let report = restore_into(&mut b, &seal_json(&payload), 0);
    assert_eq!((report.imported, report.failed), (1, 1));
    assert_eq!(b.list_items().unwrap()[0].title, "Bank");
}

/// A crafted file: a type mismatch, a duplicate ID and an over-long
/// password history are refused item by item; the rest restores.
#[test]
fn a_hostile_backup_restores_only_its_valid_items() {
    let (mut a, _) = activated_vault();
    commit_all(
        &mut a,
        vec![
            login("Mismatch", "a", "pw-a", "https://a.example"),
            login("Twice", "b", "pw-b", "https://b.example"),
            login("History", "c", "pw-c", "https://c.example"),
            note("Fine", "body"),
        ],
    );
    let mut payload = payload_of(&a);
    let items = payload["items"].as_array_mut().unwrap();
    let by_title = |items: &[serde_json::Value], t: &str| {
        items
            .iter()
            .position(|i| i["overview"]["title"] == t)
            .unwrap()
    };
    let m = by_title(items, "Mismatch");
    items[m]["overview"]["itemType"] = "secure_note".into();
    let h = by_title(items, "History");
    let old: Vec<_> = (0..=havenkeys_core::model::MAX_PASSWORD_HISTORY)
        .map(|n| serde_json::json!({ "password": format!("old-{n}"), "replacedAt": n }))
        .collect();
    items[h]["details"]["password_history"] = old.into();
    let t = by_title(items, "Twice");
    let mut twin = items[t].clone();
    twin["details"]["password"] = "an impostor".into();
    items.push(twin);

    let (mut b, _) = activated_vault();
    let report = restore_into(&mut b, &seal_json(&payload), 0);
    assert_eq!(
        (report.imported, report.failed, report.skipped_existing),
        (2, 2, 1)
    );
    let mut titles: Vec<_> = b
        .list_items()
        .unwrap()
        .iter()
        .map(|i| i.title.clone())
        .collect();
    titles.sort();
    assert_eq!(titles, ["Fine", "Twice"]);
    let twice = b
        .list_items()
        .unwrap()
        .into_iter()
        .find(|i| i.title == "Twice")
        .unwrap();
    assert_eq!(
        b.reveal(&twice.id, havenkeys_core::model::SecretField::Password)
            .unwrap()
            .expose(),
        "pw-b"
    );
}

fn tagged(
    mut input: havenkeys_core::model::ItemInput,
    tags: &[&str],
) -> havenkeys_core::model::ItemInput {
    input.tags = Some(tags.iter().map(|t| (*t).to_owned()).collect());
    input
}

#[test]
fn a_backup_keeps_every_items_tags() {
    let (mut a, _) = activated_vault();
    commit_all(
        &mut a,
        vec![
            tagged(
                login("Bank", "me", "pw", "https://bank.example"),
                &["money", "family"],
            ),
            tagged(note("Recovery", "1111"), &["work"]),
        ],
    );
    let file = backup_of(&a, BACKUP_PW);
    let (mut b, _) = activated_vault();
    assert_eq!(restore_into(&mut b, &file, 0).failed, 0);
    let mut got: Vec<(String, Vec<String>)> = b
        .list_items()
        .unwrap()
        .into_iter()
        .map(|o| (o.title.clone(), o.tags.clone()))
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            (
                "Bank".to_owned(),
                vec!["family".to_owned(), "money".to_owned()]
            ),
            ("Recovery".to_owned(), vec!["work".to_owned()]),
        ]
    );
}

#[test]
fn bitwarden_json_exports_the_first_tag_as_the_folder() {
    let (mut v, _) = activated_vault();
    commit_all(
        &mut v,
        vec![
            tagged(
                login("Bank", "me", "pw", "https://bank.example"),
                &["b", "a"],
            ),
            login("Plain", "me", "pw", "https://plain.example"),
        ],
    );
    let out = export::render(&v, ExportFormat::BitwardenJson, NOW).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
    let folders = json["folders"].as_array().unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0]["name"], "a");
    let items = json["items"].as_array().unwrap();
    let bank = items.iter().find(|i| i["name"] == "Bank").unwrap();
    let plain = items.iter().find(|i| i["name"] == "Plain").unwrap();
    assert_eq!(bank["folderId"], folders[0]["id"]);
    assert!(plain["folderId"].is_null());

    // And it comes back in as the tag.
    let parsed = import::parse(ImportSource::BitwardenJson, &out.bytes).unwrap();
    let tags: Vec<_> = parsed.items.iter().map(|i| i.input.tags.clone()).collect();
    assert!(tags.contains(&Some(vec!["a".to_owned()])));
    assert!(tags.contains(&Some(vec![])));
}

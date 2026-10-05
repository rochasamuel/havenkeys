//! Exports from other password managers go through the same staged write
//! path as `.1pux`: every parsed item seals, and importing the same file
//! twice adds nothing the second time.

mod common;

use common::{activated_vault, NOW};
use havenkeys_core::import::{self, ImportReport, ImportSource};
use havenkeys_core::vault::VaultService;

fn import_into(
    vault: &mut VaultService,
    source: ImportSource,
    file: &str,
    base: i64,
) -> ImportReport {
    let parsed = import::parse(source, file.as_bytes()).unwrap();
    let staged = vault
        .stage_import(parsed.items, parsed.report, NOW)
        .unwrap();
    let mut revision = base;
    for write in staged.writes {
        revision += 1;
        vault.commit_write(write, revision).unwrap();
    }
    staged.report
}

#[test]
fn a_chrome_export_imported_twice_adds_nothing_the_second_time() {
    let (mut vault, _sk) = activated_vault();
    let csv = "name,url,username,password,note\n\
        github.com,https://github.com/login,octo,pw,\n\
        ,https://fastmail.com/,me@fastmail.com,pw2,recovery in safe\n";
    let first = import_into(&mut vault, ImportSource::Chrome, csv, 0);
    assert_eq!((first.imported, first.logins, first.failed), (2, 2, 0));
    let second = import_into(&mut vault, ImportSource::Chrome, csv, 10);
    assert_eq!((second.imported, second.skipped_duplicates), (0, 2));
    assert_eq!(vault.list_items().unwrap().len(), 2);
}

#[test]
fn every_source_seals() {
    let bitwarden = r#"{"encrypted": false, "items": [
        {"type": 1, "name": "L", "login": {"username": "u", "password": "p",
            "uris": [{"match": 1, "uri": "https://a.example"}], "totp": "JBSWY3DPEHPK3PXP"},
            "fields": [{"name": "PIN", "value": "1", "type": 1}]},
        {"type": 2, "name": "N", "notes": "body"},
        {"type": 3, "name": "C", "card": {"number": "4111111111111111", "code": "123",
            "expMonth": "12", "expYear": "2030", "brand": "Visa"}},
        {"type": 4, "name": "I", "identity": {"firstName": "Sam"}}
    ]}"#;
    let cases = [
        (ImportSource::BitwardenJson, bitwarden.to_owned(), 4),
        (
            ImportSource::BitwardenCsv,
            "type,name,notes,fields,login_uri,login_username,login_password,login_totp\n\
             login,L,n,PIN: 1,https://a.example,u,p,JBSWY3DPEHPK3PXP\nnote,N,body,,,,,\n"
                .to_owned(),
            2,
        ),
        (
            ImportSource::Firefox,
            "url,username,password,httpRealm,timeCreated\nhttps://a.example,u,p,,1600000000000\n"
                .to_owned(),
            1,
        ),
        (
            ImportSource::KeePassXc,
            "Group,Title,Username,Password,URL,Notes,TOTP\nRoot,L,u,p,https://a.example,,JBSWY3DPEHPK3PXP\nRoot,N,,,,body,\n"
                .to_owned(),
            2,
        ),
        (
            ImportSource::LastPass,
            "url,username,password,totp,extra,name,grouping,fav\nhttps://a.example,u,p,,,L,,0\nhttp://sn,,,,body,N,,0\n"
                .to_owned(),
            2,
        ),
    ];
    for (source, file, expected) in cases {
        let (mut vault, _sk) = activated_vault();
        let report = import_into(&mut vault, source, &file, 0);
        assert_eq!(
            (report.imported, report.failed),
            (expected, 0),
            "{source:?}"
        );
        assert_eq!(vault.list_items().unwrap().len(), expected, "{source:?}");
    }
}

//! The Bank Account item (spec 2026-11-03-bank-account §4).

mod common;

use common::{activated_vault, login, note, secret, NOW};
use havenkeys_core::app_target::AppIdentity;
use havenkeys_core::bank::{BankAccountFields, BankAccountInput, BankAccountType};
use havenkeys_core::card::CardInput;
use havenkeys_core::custom_field::{FieldInput, FieldValueInput, SectionInput};
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::vault::{MatchKind, SaveAction, SaveTarget, VaultService};
use havenkeys_core::Error;
use uuid::Uuid;

fn itau() -> BankAccountInput {
    BankAccountInput {
        bank: Some("Itaú".into()),
        holder_name: Some(secret("Maria Souza")),
        account_type: Some(BankAccountType::Checking),
        branch: Some("1234".into()),
        account: SecretUpdate::Set(secret("12345")),
        account_digit: SecretUpdate::Set(secret("6")),
        ..Default::default()
    }
}

fn bank_input(title: &str, bank: BankAccountInput) -> ItemInput {
    ItemInput {
        bank: Some(bank),
        ..ItemInput::blank_for_tests(ItemType::BankAccount, title)
    }
}

fn pin_section() -> SectionInput {
    SectionInput {
        id: None,
        title: Some(secret("Online")),
        fields: vec![FieldInput {
            id: None,
            label: secret("PIN"),
            value: FieldValueInput::Password(SecretUpdate::Set(secret("4321"))),
        }],
    }
}

fn create(v: &mut VaultService, input: ItemInput) -> Uuid {
    let staged = v.stage_create(input, NOW).unwrap();
    v.commit_write(staged, 1).unwrap().unwrap().id
}

fn update(v: &mut VaultService, id: &Uuid, input: ItemInput, rev: i64) {
    let staged = v.stage_update(id, input, NOW + 1).unwrap();
    v.commit_write(staged, rev).unwrap();
}

fn fields(v: &VaultService, id: &Uuid) -> BankAccountFields {
    v.bank_fields(*id).unwrap()
}

#[test]
fn bank_round_trips() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, bank_input("Main", itau()));

    let ov = v.get_item(&id).unwrap();
    assert_eq!(ov.item_type, ItemType::BankAccount);
    assert_eq!(ov.title, "Main");
    assert!(ov.username.is_none());
    let s = ov.bank.as_ref().unwrap();
    assert_eq!(s.bank.as_deref(), Some("Itaú"));
    assert_eq!(s.branch.as_deref(), Some("1234"));
    assert_eq!(s.account_last3.as_deref(), Some("45-6"));
    let json = serde_json::to_string(&ov).unwrap();
    assert!(!json.contains("12345") && !json.contains("Maria"));

    let f = fields(&v, &id);
    assert_eq!(f.holder_name.as_ref().unwrap().expose(), "Maria Souza");
    assert_eq!(f.account.as_ref().unwrap().expose(), "12345");
    assert_eq!(f.account_digit.as_ref().unwrap().expose(), "6");
}

#[test]
fn title_falls_back_to_bank_then_default() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, bank_input("", itau()));
    assert_eq!(v.get_item(&id).unwrap().title, "Itaú");

    let no_bank = BankAccountInput {
        account: SecretUpdate::Set(secret("12345")),
        ..Default::default()
    };
    let id = create(&mut v, bank_input("  ", no_bank));
    assert_eq!(v.get_item(&id).unwrap().title, "Bank account");
}

#[test]
fn empty_bank_account_is_rejected() {
    let (v, _) = activated_vault();
    let e = v.stage_create(bank_input(" ", BankAccountInput::default()), NOW);
    assert!(matches!(e, Err(Error::InvalidInput(_))));
    // A title alone is enough.
    assert!(v
        .stage_create(bank_input("Main", BankAccountInput::default()), NOW)
        .is_ok());
    let only_holder = BankAccountInput {
        holder_name: Some(secret("Maria")),
        ..Default::default()
    };
    assert!(v.stage_create(bank_input("", only_holder), NOW).is_err());
}

#[test]
fn shape_rules() {
    let (mut v, _) = activated_vault();
    let with = |f: fn(&mut ItemInput)| {
        let mut i = bank_input("B", itau());
        f(&mut i);
        i
    };
    let refused: Vec<ItemInput> = vec![
        with(|i| i.username = Some("me".into())),
        with(|i| i.totp = SecretUpdate::Set(secret("JBSWY3DPEHPK3PXP"))),
        with(|i| i.content = SecretUpdate::Set(secret("x"))),
        with(|i| i.card = Some(CardInput::default())),
        with(|i| i.identity = Some(Default::default())),
        with(|i| i.sign_in_with = Some(serde_json::from_str(r#"{"provider":"google"}"#).unwrap())),
        // A bank account without its values, and a login with them.
        ItemInput::blank_for_tests(ItemType::BankAccount, "B"),
        ItemInput {
            bank: Some(itau()),
            ..ItemInput::blank_for_tests(ItemType::Login, "L")
        },
        ItemInput {
            bank: Some(itau()),
            ..ItemInput::blank_for_tests(ItemType::SecureNote, "N")
        },
    ];
    for (n, input) in refused.into_iter().enumerate() {
        let e = v.stage_create(input, NOW);
        assert!(matches!(e, Err(Error::InvalidInput(_))), "case {n}");
    }

    let ok = ItemInput {
        urls: vec![UrlRule {
            url: "itau.com.br".into(),
            match_type: MatchType::Domain,
        }],
        password: SecretUpdate::Set(secret("hunter2")),
        notes: SecretUpdate::Set(secret("agency note")),
        sections: Some(vec![pin_section()]),
        ..bank_input("B", itau())
    };
    let id = create(&mut v, ok);
    let ov = v.get_item(&id).unwrap();
    assert_eq!(ov.urls.len(), 1);
    assert!(ov.has_password && ov.has_notes);
    assert_eq!(fields(&v, &id).sections.len(), 1);
    let f = fields(&v, &id);
    assert_eq!(f.password.unwrap().expose(), "hunter2");
    assert_eq!(f.notes.unwrap().expose(), "agency note");
}

#[test]
fn account_keep_set_clear() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, bank_input("Main", itau()));

    let keep = BankAccountInput {
        account: SecretUpdate::Keep,
        account_digit: SecretUpdate::Keep,
        ..itau()
    };
    update(&mut v, &id, bank_input("Main", keep), 2);
    let f = fields(&v, &id);
    assert_eq!(f.account.unwrap().expose(), "12345");

    let set = BankAccountInput {
        account: SecretUpdate::Set(secret("98765")),
        account_digit: SecretUpdate::Keep,
        ..itau()
    };
    update(&mut v, &id, bank_input("Main", set), 3);
    let f = fields(&v, &id);
    assert_eq!(f.account.unwrap().expose(), "98765");
    assert_eq!(f.account_digit.unwrap().expose(), "6");

    let clear = BankAccountInput {
        account: SecretUpdate::Clear,
        account_digit: SecretUpdate::Clear,
        ..itau()
    };
    update(&mut v, &id, bank_input("Main", clear), 4);
    assert!(fields(&v, &id).account.is_none());
    let ov = v.get_item(&id).unwrap();
    assert!(ov.bank.as_ref().unwrap().account_last3.is_none());
}

#[test]
fn type_change_into_or_out_of_bank_is_refused() {
    let (mut v, _) = activated_vault();
    let bank = create(&mut v, bank_input("Main", itau()));
    let lg = create(&mut v, login("L", "u", "p", "example.com"));
    assert!(v.stage_update(&bank, note("Main", "x"), NOW + 1).is_err());
    assert!(v
        .stage_update(&lg, bank_input("L", itau()), NOW + 1)
        .is_err());
}

#[test]
fn search_finds_bank_and_branch_not_full_account() {
    let (mut v, _) = activated_vault();
    let input = BankAccountInput {
        bank: Some("Itau".into()),
        ..itau()
    };
    let id = create(&mut v, bank_input("Salary", input));
    for q in ["itau", "1234"] {
        let hits = v.search(q).unwrap();
        assert_eq!(hits.len(), 1, "{q}");
        assert_eq!(hits[0].id, id);
    }
    assert!(v.search("12345").unwrap().is_empty());
    assert!(v.search("45-6").unwrap().is_empty());
}

#[test]
fn pasted_account_with_digit_is_split() {
    let (mut v, _) = activated_vault();
    let input = BankAccountInput {
        account: SecretUpdate::Set(secret("12345-6")),
        account_digit: SecretUpdate::Keep,
        ..itau()
    };
    let id = create(&mut v, bank_input("Main", input));
    let f = fields(&v, &id);
    assert_eq!(f.account.unwrap().expose(), "12345");
    assert_eq!(f.account_digit.unwrap().expose(), "6");
}

#[test]
fn a_bank_fields_read_is_denied_for_other_items() {
    let (mut v, _) = activated_vault();
    let lg = create(&mut v, login("L", "u", "p", "example.com"));
    assert_eq!(v.bank_fields(lg).err(), Some(Error::Denied));
}

#[test]
fn only_logins_and_bank_accounts_are_site_bound() {
    assert!(ItemType::Login.is_site_bound());
    assert!(ItemType::BankAccount.is_site_bound());
    assert!(!ItemType::SecureNote.is_site_bound());
    assert!(!ItemType::Card.is_site_bound());
    assert!(!ItemType::Identity.is_site_bound());
}

#[test]
fn custom_fields_read_back_through_item_sections_and_item_field() {
    let (mut v, _) = activated_vault();
    let input = ItemInput {
        sections: Some(vec![pin_section()]),
        ..bank_input("Main", itau())
    };
    let id = create(&mut v, input);
    let sections = v.item_sections(&id).unwrap();
    assert_eq!(sections.len(), 1);
    let field = &sections[0].fields[0];
    assert_eq!(field.label.expose(), "PIN");
    let same = v.item_field(&id, &field.id).unwrap();
    assert_eq!(same.id, field.id);
    assert!(v.item_field(&id, &Uuid::new_v4()).is_err());
}

// ---------------------------------------------------------------- site binding

const ITAU: &str = "https://www.itau.com.br/";

fn itau_bank(password: &str) -> ItemInput {
    ItemInput {
        urls: vec![UrlRule {
            url: "https://www.itau.com.br".into(),
            match_type: MatchType::Domain,
        }],
        password: SecretUpdate::Set(secret(password)),
        notes: SecretUpdate::Set(secret("agency note")),
        sections: Some(vec![pin_section()]),
        ..bank_input("Itaú PF", itau())
    }
}

#[test]
fn bank_matches_its_site_ranked_with_logins() {
    let (mut v, _) = activated_vault();
    let lg = create(
        &mut v,
        login("Itaú old", "maria", "pw", "https://www.itau.com.br"),
    );
    let bank = create(&mut v, itau_bank("bank-pw"));
    let m = v.find_matches(ITAU, None, NOW).unwrap();
    assert_eq!(m.len(), 2);
    let b = m.iter().find(|s| s.id == bank).unwrap();
    assert_eq!(b.kind, MatchKind::BankAccount);
    assert!(b.username.is_none());
    let summary = b.bank.as_ref().unwrap();
    assert_eq!(summary.branch.as_deref(), Some("1234"));
    assert_eq!(summary.account_last3.as_deref(), Some("45-6"));
    let l = m.iter().find(|s| s.id == lg).unwrap();
    assert_eq!(l.kind, MatchKind::Login);
    assert!(l.bank.is_none());
    let json = serde_json::to_string(&m).unwrap();
    assert!(!json.contains("12345") && !json.contains("bank-pw") && !json.contains("Maria"));
    assert!(v.matches_page(&bank, ITAU, None).unwrap());
}

#[test]
fn bank_fill_returns_values_and_password_only() {
    let (mut v, _) = activated_vault();
    let bank = create(&mut v, itau_bank("bank-pw"));
    let c = v.fill_for_page(&bank, ITAU, None, NOW).unwrap();
    assert!(c.username.is_none());
    assert_eq!(c.password.unwrap().expose(), "bank-pw");
    let b = c.bank.unwrap();
    assert_eq!(b.branch.as_deref(), Some("1234"));
    assert!(b.branch_digit.is_none());
    assert_eq!(b.account.unwrap().expose(), "12345");
    assert_eq!(b.account_digit.unwrap().expose(), "6");

    // A login's reply has no bank values.
    let lg = create(&mut v, login("L", "u", "p", "https://www.itau.com.br"));
    assert!(v
        .fill_for_page(&lg, ITAU, None, NOW)
        .unwrap()
        .bank
        .is_none());
}

#[test]
fn bank_fill_carries_whichever_number_the_item_has() {
    let (mut v, _) = activated_vault();
    let conta_only = create(
        &mut v,
        ItemInput {
            bank: Some(BankAccountInput {
                branch: None,
                ..itau()
            }),
            ..itau_bank("bank-pw")
        },
    );
    let c = v.fill_for_page(&conta_only, ITAU, None, NOW).unwrap();
    assert_eq!(c.password.unwrap().expose(), "bank-pw");
    let b = c.bank.unwrap();
    assert!(b.branch.is_none() && b.branch_digit.is_none());
    assert_eq!(b.account.unwrap().expose(), "12345");
    assert_eq!(b.account_digit.unwrap().expose(), "6");

    let branch_only = create(
        &mut v,
        ItemInput {
            bank: Some(BankAccountInput {
                branch: Some("1234-5".into()),
                account: SecretUpdate::Keep,
                account_digit: SecretUpdate::Keep,
                ..itau()
            }),
            ..itau_bank("bank-pw")
        },
    );
    let b = v
        .fill_for_page(&branch_only, ITAU, None, NOW)
        .unwrap()
        .bank
        .unwrap();
    assert_eq!(b.branch.as_deref(), Some("1234"));
    assert_eq!(b.branch_digit.as_deref(), Some("5"));
    assert!(b.account.is_none() && b.account_digit.is_none());

    // Neither number: the password alone.
    let neither = create(
        &mut v,
        ItemInput {
            bank: Some(BankAccountInput {
                branch: None,
                account: SecretUpdate::Keep,
                account_digit: SecretUpdate::Keep,
                ..itau()
            }),
            ..itau_bank("bank-pw")
        },
    );
    let c = v.fill_for_page(&neither, ITAU, None, NOW).unwrap();
    assert!(c.bank.is_none());
    assert_eq!(c.password.unwrap().expose(), "bank-pw");
}

#[test]
fn fill_for_app_serves_bank_to_bound_app_only() {
    let (mut v, _) = activated_vault();
    let bank = create(&mut v, itau_bank("bank-pw"));
    let app = AppIdentity::new("com.itau", &[vec![1; 32]]).unwrap();
    let staged = v.stage_bind_app(&bank, &app, NOW).unwrap();
    v.commit_write(staged, 2).unwrap();
    assert_eq!(fields(&v, &bank).app_bindings.len(), 1);

    let m = v.matches_for_app(&app, &[], NOW).unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].kind, MatchKind::BankAccount);
    let c = v.fill_for_app(&bank, &app, &[]).unwrap();
    assert!(c.username.is_none());
    assert_eq!(c.password.unwrap().expose(), "bank-pw");
    assert_eq!(c.bank.unwrap().account.unwrap().expose(), "12345");

    let wrong = AppIdentity::new("com.itau", &[vec![2; 32]]).unwrap();
    assert!(v.matches_for_app(&wrong, &[], NOW).unwrap().is_empty());
    assert_eq!(
        v.fill_for_app(&bank, &wrong, &[]).err(),
        Some(Error::Denied)
    );
    assert_eq!(
        v.totp_for_app(&bank, &app, &[], 59).err(),
        Some(Error::Denied)
    );
    // Verified by the site, too.
    let hosts = vec!["www.itau.com.br".to_string()];
    assert!(v.fill_for_app(&bank, &wrong, &hosts).is_ok());
    assert!(v.login_hosts().unwrap().contains(&"www.itau.com.br".into()));
    assert_eq!(v.search_logins("itaú").unwrap()[0].id, bank);
}

#[test]
fn check_login_unchanged_for_bank_password() {
    let (mut v, _) = activated_vault();
    let bank = create(&mut v, itau_bank("bank-pw"));
    let check =
        |user: Option<&str>, pw: &str| v.check_login(ITAU, None, user, &secret(pw), None).unwrap();
    assert_eq!(check(None, "bank-pw"), SaveAction::Unchanged);
    assert_eq!(check(Some(" "), "bank-pw"), SaveAction::Unchanged);
    assert_eq!(check(None, "other"), SaveAction::Add);
    assert_eq!(check(Some("maria"), "other"), SaveAction::Add);
    // Whatever the form captured as a username (an agência, a conta).
    assert_eq!(check(Some("maria"), "bank-pw"), SaveAction::Unchanged);
    assert_eq!(check(Some("12345"), "bank-pw"), SaveAction::Unchanged);
    // A change-password form naming the bank's password never updates it.
    assert_eq!(
        v.check_login(ITAU, None, None, &secret("new"), Some(&secret("bank-pw")))
            .unwrap(),
        SaveAction::Add
    );
    // Nor may a save update be aimed at it.
    let e = v.stage_save_login(
        ITAU,
        None,
        None,
        secret("new"),
        SaveTarget::Update(&bank),
        NOW,
    );
    assert_eq!(e.err(), Some(Error::Denied));
}

#[test]
fn health_skips_bank_accounts() {
    let (mut v, _) = activated_vault();
    create(&mut v, itau_bank("123456"));
    create(&mut v, itau_bank("123456"));
    let report = havenkeys_core::health::compute(&v.health_snapshot().unwrap(), NOW);
    assert!(report.issues.is_empty());
    assert_eq!(report.counts.weak + report.counts.reused, 0);
}

#[test]
fn lookalike_names_a_bank_accounts_site() {
    let (mut v, _) = activated_vault();
    create(&mut v, itau_bank("bank-pw"));
    // "itаu" with a Cyrillic "а".
    let fake = url::Url::parse("https://it\u{0430}u.com.br/login").unwrap();
    assert_eq!(
        v.lookalike_for_page(fake.as_str(), None)
            .unwrap()
            .as_deref(),
        Some("itau.com.br")
    );
    assert_eq!(v.lookalike_for_page(ITAU, None).unwrap(), None);
}

fn exported(
    format: havenkeys_core::export::ExportFormat,
) -> (String, havenkeys_core::export::ExportSummary) {
    let (mut v, _) = activated_vault();
    create(&mut v, itau_bank("bank-pw"));
    let r = havenkeys_core::export::render(&v, format, NOW).unwrap();
    (String::from_utf8(r.bytes.to_vec()).unwrap(), r.summary)
}

#[test]
fn bitwarden_export_writes_a_bank_account_as_a_login() {
    use havenkeys_core::export::ExportFormat;
    let (json, summary) = exported(ExportFormat::BitwardenJson);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let item = &v["items"][0];
    assert_eq!(item["type"], 1);
    assert_eq!(item["login"]["password"], "bank-pw");
    assert_eq!(item["login"]["uris"][0]["uri"], "https://www.itau.com.br/");
    assert!(item["login"]["totp"].is_null());
    let field = |name: &str| {
        item["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == name)
            .unwrap_or_else(|| panic!("{name}"))
    };
    assert_eq!(
        (
            field("Bank")["value"].as_str(),
            field("Bank")["type"].as_u64()
        ),
        (Some("Itaú"), Some(0))
    );
    assert_eq!(field("Agência")["value"], "1234");
    assert_eq!(field("Conta")["value"], "12345-6");
    assert_eq!(field("Conta")["type"], 1);
    assert_eq!(field("Titular")["value"], "Maria Souza");
    assert_eq!(field("Titular")["type"], 0);
    assert_eq!(field("Tipo de conta")["value"], "corrente");
    assert_eq!(field("PIN")["value"], "4321");
    assert_eq!(summary.bank_accounts, 1);
}

#[test]
fn csv_export_writes_a_bank_account_as_a_login_row() {
    use havenkeys_core::export::ExportFormat;
    let (csv, summary) = exported(ExportFormat::Csv);
    let mut r = ::csv::Reader::from_reader(csv.as_bytes());
    let row = r.records().next().unwrap().unwrap();
    assert_eq!(&row[0], "Itaú PF");
    assert_eq!(&row[1], "https://www.itau.com.br/");
    assert_eq!(&row[2], "");
    assert_eq!(&row[3], "bank-pw");
    assert_eq!(
        &row[4],
        "agency note\nBank: Itaú\nAgência: 1234\nConta: 12345-6\nTitular: Maria Souza\nTipo: corrente"
    );
    assert_eq!(summary.bank_accounts, 1);
    assert_eq!(summary.items_left_out, 0);
    assert_eq!(summary.custom_fields_left_out, 1);
}

#[test]
fn csv_export_leaves_out_an_absent_holder_and_type() {
    use havenkeys_core::export::{render, ExportFormat};
    let (mut v, _) = activated_vault();
    let bare = BankAccountInput {
        holder_name: None,
        account_type: None,
        ..itau()
    };
    create(&mut v, bank_input("B", bare));
    let r = render(&v, ExportFormat::Csv, NOW).unwrap();
    let csv = String::from_utf8(r.bytes.to_vec()).unwrap();
    let row = ::csv::Reader::from_reader(csv.as_bytes())
        .records()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(&row[4], "Bank: Itaú\nAgência: 1234\nConta: 12345-6");
}

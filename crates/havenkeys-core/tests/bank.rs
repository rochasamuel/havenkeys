//! The Bank Account item (spec 2026-11-03-bank-account §4).

mod common;

use common::{activated_vault, login, note, secret, NOW};
use havenkeys_core::bank::{BankAccountFields, BankAccountInput};
use havenkeys_core::card::CardInput;
use havenkeys_core::custom_field::{FieldInput, FieldValueInput, SectionInput};
use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::vault::VaultService;
use havenkeys_core::Error;
use uuid::Uuid;

fn itau() -> BankAccountInput {
    BankAccountInput {
        bank: Some("Itaú".into()),
        holder_name: Some(secret("Maria Souza")),
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

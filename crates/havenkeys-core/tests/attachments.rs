//! Attachments on the item overview (spec 2026-10-13-encrypted-attachments).

mod common;

use common::{activated_vault, login, note, secret, NOW};
use havenkeys_core::attachment::{
    AttachmentChange, AttachmentMeta, FileEdits, MAX_ATTACHMENTS_PER_ITEM, MAX_ATTACHMENT_BYTES,
};
use havenkeys_core::card::{CardExpiry, CardInput};
use havenkeys_core::identity::IdentityFields;
use havenkeys_core::model::{ItemInput, ItemOverview, ItemType, SecretField, SecretUpdate};
use havenkeys_core::vault::VaultService;
use havenkeys_core::Error;
use uuid::Uuid;

fn create(v: &mut VaultService, input: ItemInput) -> Uuid {
    let staged = v.stage_create(input, NOW).unwrap();
    let id = staged.item_id;
    v.commit_write(staged, 1).unwrap();
    id
}

/// Seal `count` files for `id` and add them in one write, committed at `rev`.
fn attach(v: &mut VaultService, id: &Uuid, count: usize, rev: i64) -> Vec<AttachmentMeta> {
    let metas: Vec<AttachmentMeta> = (0..count)
        .map(|n| {
            v.seal_attachment(id, &format!("f{n}.pdf"), b"%PDF-1.7 body", NOW)
                .unwrap()
                .meta
        })
        .collect();
    let staged = v
        .stage_attachments(id, AttachmentChange::Add(metas.clone()), NOW + 1)
        .unwrap();
    v.commit_write(staged, rev).unwrap();
    metas
}

fn ids(overview: &ItemOverview) -> Vec<Uuid> {
    overview.attachments.iter().map(|m| m.id).collect()
}

#[test]
fn seal_then_open_round_trips() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let sealed = v
        .seal_attachment(&id, "../a/scan.pdf", b"%PDF-1.7 body", NOW)
        .unwrap();
    assert_eq!(sealed.meta.name, "scan.pdf");
    assert_eq!(sealed.meta.mime, "application/pdf");
    assert_eq!(sealed.meta.size, 13);
    assert_eq!(sealed.meta.created_at, NOW);
    let opened = v.open_attachment(&id, &sealed.meta, &sealed.blob).unwrap();
    assert_eq!(opened.expose(), b"%PDF-1.7 body");
}

#[test]
fn a_tampered_blob_or_wrong_item_fails_closed() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let sealed = v.seal_attachment(&id, "a.bin", b"secret", NOW).unwrap();

    let mut flipped = sealed.blob.clone();
    flipped[20] ^= 1;
    assert!(matches!(
        v.open_attachment(&id, &sealed.meta, &flipped),
        Err(Error::Decryption)
    ));
    assert!(matches!(
        v.open_attachment(&Uuid::new_v4(), &sealed.meta, &sealed.blob),
        Err(Error::Decryption)
    ));
}

#[test]
fn oversize_is_refused_before_sealing() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let big = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
    assert!(matches!(
        v.seal_attachment(&id, "big.bin", &big, NOW),
        Err(Error::InvalidInput("Files can be up to 5 MB"))
    ));
}

#[test]
fn stage_add_lists_every_id_and_keeps_details() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let metas: Vec<_> = (0..2)
        .map(|_| v.seal_attachment(&id, "a.bin", b"x", NOW).unwrap().meta)
        .collect();
    let want: Vec<Uuid> = metas.iter().map(|m| m.id).collect();

    let staged = v
        .stage_attachments(&id, AttachmentChange::Add(metas), NOW + 1)
        .unwrap();
    assert_eq!(staged.attachment_ids, Some(want.clone()));
    v.commit_write(staged, 2).unwrap();

    assert_eq!(ids(&v.get_item(&id).unwrap()), want);
    assert_eq!(v.reveal(&id, SecretField::Password).unwrap().expose(), "pw");
    assert_eq!(v.get_item(&id).unwrap().updated_at, NOW + 1);
}

#[test]
fn stage_add_refuses_the_21st() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    attach(&mut v, &id, MAX_ATTACHMENTS_PER_ITEM, 2);
    let one_more = v.seal_attachment(&id, "a.bin", b"x", NOW).unwrap().meta;
    assert!(matches!(
        v.stage_attachments(&id, AttachmentChange::Add(vec![one_more]), NOW),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn stage_remove_drops_one() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let metas = attach(&mut v, &id, 2, 2);
    let staged = v
        .stage_attachments(&id, AttachmentChange::Remove(metas[0].id), NOW + 2)
        .unwrap();
    assert_eq!(staged.attachment_ids, Some(vec![metas[1].id]));
    v.commit_write(staged, 3).unwrap();
    assert_eq!(ids(&v.get_item(&id).unwrap()), vec![metas[1].id]);
    assert!(matches!(
        v.stage_attachments(&id, AttachmentChange::Remove(metas[0].id), NOW),
        Err(Error::NotFound)
    ));
}

fn seal(v: &VaultService, id: &Uuid, n: usize) -> Vec<AttachmentMeta> {
    (0..n)
        .map(|_| v.seal_attachment(id, "a.bin", b"x", NOW).unwrap().meta)
        .collect()
}

#[test]
fn update_with_adds_and_removes_in_one_write() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let kept = attach(&mut v, &id, 3, 2);
    let added = seal(&v, &id, 2);
    let want: Vec<Uuid> = [kept[0].id, kept[2].id]
        .into_iter()
        .chain(added.iter().map(|m| m.id))
        .collect();
    let files = FileEdits {
        added,
        removed: vec![kept[1].id],
    };

    let input = login("GitHub 2", "me", "pw", "github.com");
    let staged = v.stage_update_with(&id, input, files, NOW + 3).unwrap();
    assert_eq!(staged.attachment_ids, Some(want.clone()));
    v.commit_write(staged, 3).unwrap();

    let item = v.get_item(&id).unwrap();
    assert_eq!(ids(&item), want);
    assert_eq!(item.title, "GitHub 2");
    assert_eq!(v.reveal(&id, SecretField::Password).unwrap().expose(), "pw");
}

#[test]
fn a_plain_edit_sends_no_attachment_ids() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    attach(&mut v, &id, 2, 2);
    let input = login("GitHub 2", "me", "pw", "github.com");
    let staged = v
        .stage_update_with(&id, input, FileEdits::default(), NOW + 3)
        .unwrap();
    assert_eq!(staged.attachment_ids, None);
}

#[test]
fn removing_every_file_sends_an_empty_list() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let metas = attach(&mut v, &id, 2, 2);
    let files = FileEdits {
        added: vec![],
        removed: ids_of(&metas),
    };
    let input = login("GitHub", "me", "pw", "github.com");
    let staged = v.stage_update_with(&id, input, files, NOW + 3).unwrap();
    assert_eq!(staged.attachment_ids, Some(vec![]));
}

#[test]
fn removing_an_unknown_file_is_not_found() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let files = FileEdits {
        added: vec![],
        removed: vec![Uuid::new_v4()],
    };
    let input = login("GitHub", "me", "pw", "github.com");
    assert!(matches!(
        v.stage_update_with(&id, input, files, NOW + 3),
        Err(Error::NotFound)
    ));
}

#[test]
fn the_21st_file_is_refused() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    attach(&mut v, &id, MAX_ATTACHMENTS_PER_ITEM, 2);
    let files = FileEdits {
        added: seal(&v, &id, 1),
        removed: vec![],
    };
    let input = login("GitHub", "me", "pw", "github.com");
    assert!(matches!(
        v.stage_update_with(&id, input, files, NOW + 3),
        Err(Error::InvalidInput("An item holds up to 20 files"))
    ));
}

#[test]
fn create_with_files_uses_the_given_id() {
    let (mut v, _) = activated_vault();
    let id = Uuid::new_v4();
    let added = seal(&v, &id, 2);
    let want = ids_of(&added);
    let files = FileEdits {
        added,
        removed: vec![],
    };
    let input = login("GitHub", "me", "pw", "github.com");
    let staged = v.stage_create_with(id, input, files, NOW).unwrap();
    assert_eq!(staged.item_id, id);
    assert_eq!(staged.attachment_ids, Some(want.clone()));
    v.commit_write(staged, 1).unwrap();
    assert_eq!(ids(&v.get_item(&id).unwrap()), want);
}

fn card_input() -> ItemInput {
    ItemInput {
        item_type: ItemType::Card,
        card: Some(CardInput {
            cardholder_name: Some(secret("Sam")),
            brand: None,
            number: SecretUpdate::Set(secret("5200 8282 8282 8210")),
            verification_number: SecretUpdate::Keep,
            expiry: Some(CardExpiry::new(2033, 11).unwrap()),
            notes: None,
        }),
        ..ItemInput::blank_for_tests(ItemType::Card, "Visa")
    }
}

fn identity_input() -> ItemInput {
    ItemInput {
        identity: Some(IdentityFields::default()),
        ..ItemInput::blank_for_tests(ItemType::Identity, "")
    }
}

fn identity(v: &mut VaultService) -> Uuid {
    let staged = v
        .stage_identity_if_missing("user@example.com", NOW)
        .unwrap()
        .unwrap();
    let id = staged.item_id;
    v.commit_write(staged, 1).unwrap();
    id
}

#[test]
fn every_edit_path_keeps_the_manifest() {
    let (mut v, _) = activated_vault();
    let login_id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let note_id = create(&mut v, note("Wifi", "hunter2"));
    let card_id = create(&mut v, card_input());
    let identity_id = identity(&mut v);
    let edits = [
        (login_id, login("GitHub 2", "me", "pw2", "github.com")),
        (note_id, note("Wifi 2", "hunter3")),
        (card_id, card_input()),
        (identity_id, identity_input()),
    ];
    for (id, input) in edits {
        let metas = attach(&mut v, &id, 2, 5);
        let staged = v.stage_update(&id, input, NOW + 9).unwrap();
        assert_eq!(staged.attachment_ids, None);
        v.commit_write(staged, 6).unwrap();
        assert_eq!(ids(&v.get_item(&id).unwrap()), ids_of(&metas));
        if id == identity_id {
            continue; // The Identity is never trashed.
        }

        let trashed = v.stage_trash(&id, NOW + 10).unwrap();
        assert_eq!(trashed.attachment_ids, None);
        v.commit_write(trashed, 7).unwrap();
        let restored = v.stage_restore_trashed(&id).unwrap();
        assert_eq!(restored.attachment_ids, None);
        v.commit_write(restored, 8).unwrap();
        assert_eq!(ids(&v.get_item(&id).unwrap()), ids_of(&metas));
    }
}

fn ids_of(metas: &[AttachmentMeta]) -> Vec<Uuid> {
    metas.iter().map(|m| m.id).collect()
}

#[test]
fn a_trashed_items_attachment_can_be_found_and_staged() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let metas = attach(&mut v, &id, 2, 2);
    let trashed = v.stage_trash(&id, NOW).unwrap();
    v.commit_write(trashed, 3).unwrap();

    assert_eq!(
        v.attachment_meta(&id, &metas[1].id).unwrap().id,
        metas[1].id
    );
    assert!(matches!(
        v.attachment_meta(&id, &Uuid::new_v4()),
        Err(Error::NotFound)
    ));
    let staged = v
        .stage_attachments(&id, AttachmentChange::Remove(metas[0].id), NOW)
        .unwrap();
    assert_eq!(staged.attachment_ids, Some(vec![metas[1].id]));
}

/// The overview as 0.29 knows it: unknown fields ride along in `extra`.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OldOverview {
    id: Uuid,
    item_type: ItemType,
    title: String,
    has_password: bool,
    has_totp: bool,
    has_notes: bool,
    created_at: i64,
    updated_at: i64,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[test]
fn an_overview_manifest_survives_an_edit_by_an_older_build() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    let metas = attach(&mut v, &id, 2, 2);

    let json = serde_json::to_vec(&v.get_item(&id).unwrap()).unwrap();
    let mut old: OldOverview = serde_json::from_slice(&json).unwrap();
    old.title = "Renamed".into();
    let back: ItemOverview = serde_json::from_slice(&serde_json::to_vec(&old).unwrap()).unwrap();
    assert_eq!(back.title, "Renamed");
    assert_eq!(ids(&back), ids_of(&metas));
}

/// A field a newer build adds to a file's record is kept, not refused, and
/// rides along when this build edits the item.
#[test]
fn an_unknown_attachment_field_survives_an_edit() {
    let (mut v, _) = activated_vault();
    let id = create(&mut v, login("GitHub", "me", "pw", "github.com"));
    attach(&mut v, &id, 1, 2);

    let mut json = serde_json::to_value(v.get_item(&id).unwrap()).unwrap();
    json["attachments"][0]["fromTheFuture"] = serde_json::json!(7);
    let mut edited: ItemOverview = serde_json::from_value(json).unwrap();
    edited.title = "Renamed".into();
    let back = serde_json::to_value(&edited).unwrap();
    assert_eq!(back["attachments"][0]["fromTheFuture"], 7);
}

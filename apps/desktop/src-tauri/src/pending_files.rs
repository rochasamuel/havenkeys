//! Files the user picked for an item that is not saved yet. The bytes stay
//! here, in Rust; the renderer holds only a token per file and hands the
//! tokens back with the save (spec 2026-10-13-encrypted-attachments).

use crate::state::CmdResult;
use havenkeys_client::NewFile;
use havenkeys_core::Error;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Default)]
pub struct PendingFiles(Mutex<HashMap<Uuid, NewFile>>);

impl PendingFiles {
    fn files(&self) -> MutexGuard<'_, HashMap<Uuid, NewFile>> {
        // A panic elsewhere must not stop a lock from emptying the store.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn insert(&self, name: String, bytes: Zeroizing<Vec<u8>>) -> Uuid {
        let token = Uuid::new_v4();
        self.files().insert(token, NewFile { name, bytes });
        token
    }

    /// Take the files out for a save. Every token is consumed, so a save that
    /// fails does not leave files behind; an unknown one refuses the save.
    pub fn take(&self, tokens: &[Uuid]) -> CmdResult<Vec<NewFile>> {
        let mut files = self.files();
        let taken: Vec<_> = tokens.iter().map(|t| files.remove(t)).collect();
        match taken.into_iter().collect::<Option<Vec<_>>>() {
            Some(found) => Ok(found),
            None => Err(
                Error::InvalidInput("This file is no longer available. Attach it again.").into(),
            ),
        }
    }

    pub fn discard(&self, tokens: &[Uuid]) {
        let mut files = self.files();
        for token in tokens {
            files.remove(token);
        }
    }

    pub fn clear(&self) {
        self.files().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(b: &[u8]) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(b.to_vec())
    }

    #[test]
    fn insert_then_take_returns_bytes_once() {
        let store = PendingFiles::default();
        let token = store.insert("a.png".into(), bytes(b"abc"));
        let files = store.take(&[token]).unwrap();
        assert_eq!(
            (files[0].name.as_str(), files[0].bytes.as_slice()),
            ("a.png", &b"abc"[..])
        );
        assert!(store.take(&[token]).is_err());
    }

    #[test]
    fn unknown_token_is_refused() {
        let store = PendingFiles::default();
        let kept = store.insert("a".into(), bytes(b"1"));
        let err = store.take(&[kept, Uuid::new_v4()]).err().unwrap();
        assert!(err.message.contains("no longer available"));
        // The known token was consumed too: a save never half-keeps files.
        assert!(store.take(&[kept]).is_err());
    }

    #[test]
    fn discard_clears_tokens() {
        let store = PendingFiles::default();
        let (a, b) = (
            store.insert("a".into(), bytes(b"1")),
            store.insert("b".into(), bytes(b"2")),
        );
        store.discard(&[a, Uuid::new_v4()]);
        assert!(store.take(&[a]).is_err());
        assert_eq!(store.take(&[b]).unwrap().len(), 1);
    }

    #[test]
    fn lock_clears_pending_files() {
        let store = PendingFiles::default();
        let token = store.insert("a".into(), bytes(b"1"));
        store.clear();
        assert!(store.take(&[token]).is_err());
    }
}

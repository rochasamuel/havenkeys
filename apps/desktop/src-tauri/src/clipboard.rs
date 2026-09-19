//! Clipboard handling with automatic clearing.
//!
//! * Copying happens here, in Rust, so secrets never pass through the WebView.
//! * We remember only a SHA-256 digest of what we copied, and clear the
//!   clipboard later only if it still holds that exact value — never
//!   clobbering something the user copied afterwards.
//! * Where supported, the value is flagged to be excluded from clipboard
//!   history/managers.

use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeroize::Zeroizing;

#[derive(Default)]
struct Inner {
    // Kept alive: on X11/Wayland the owning process must stay around to serve
    // the selection.
    clipboard: Option<arboard::Clipboard>,
    generation: u64,
    owned_digest: Option<[u8; 32]>,
}

#[derive(Clone, Default)]
pub struct ClipboardGuard {
    inner: Arc<Mutex<Inner>>,
}

fn digest(text: &str) -> [u8; 32] {
    Sha256::digest(text.as_bytes()).into()
}

#[derive(Debug)]
pub struct ClipboardError;

impl ClipboardGuard {
    /// Copy `text`, then clear it after `clear_after` if still unchanged.
    pub fn copy(&self, text: &str, clear_after: Duration) -> Result<(), ClipboardError> {
        let generation = {
            let mut inner = self.inner.lock().map_err(|_| ClipboardError)?;
            if inner.clipboard.is_none() {
                inner.clipboard = Some(arboard::Clipboard::new().map_err(|_| ClipboardError)?);
            }
            let cb = inner.clipboard.as_mut().ok_or(ClipboardError)?;
            set_sensitive_text(cb, text)?;
            inner.generation = inner.generation.wrapping_add(1);
            inner.owned_digest = Some(digest(text));
            inner.generation
        };

        let this = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(clear_after);
            this.clear_if_owned(Some(generation));
        });
        Ok(())
    }

    /// Clear the clipboard if it still contains a value we placed there.
    /// With `Some(generation)`, only if no newer copy happened since.
    pub fn clear_if_owned(&self, generation: Option<u64>) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if generation.is_some_and(|g| g != inner.generation) {
            return;
        }
        let Some(expected) = inner.owned_digest.take() else {
            return;
        };
        let Some(cb) = inner.clipboard.as_mut() else {
            return;
        };
        let current = cb.get_text().ok().map(Zeroizing::new);
        if current.as_deref().is_some_and(|c| digest(c) == expected) {
            let _ = cb.clear();
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn set_sensitive_text(cb: &mut arboard::Clipboard, text: &str) -> Result<(), ClipboardError> {
    use arboard::SetExtLinux;
    cb.set()
        .exclude_from_history()
        .text(text)
        .map_err(|_| ClipboardError)
}

#[cfg(target_os = "windows")]
fn set_sensitive_text(cb: &mut arboard::Clipboard, text: &str) -> Result<(), ClipboardError> {
    use arboard::SetExtWindows;
    cb.set()
        .exclude_from_history()
        .exclude_from_cloud()
        .exclude_from_monitoring()
        .text(text)
        .map_err(|_| ClipboardError)
}

#[cfg(target_os = "macos")]
fn set_sensitive_text(cb: &mut arboard::Clipboard, text: &str) -> Result<(), ClipboardError> {
    use arboard::SetExtApple;
    cb.set()
        .exclude_from_history()
        .text(text)
        .map_err(|_| ClipboardError)
}

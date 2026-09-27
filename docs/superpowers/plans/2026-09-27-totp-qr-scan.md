# TOTP from a QR Code — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One QR-icon button in the desktop editor's one-time codes row that
reads a TOTP setup QR code from the clipboard image, or else from the screen,
and saves it without the secret ever entering the WebView.

**Architecture:** A new Rust module `qr_scan.rs` decodes QR codes
(clipboard first, then every monitor) and keeps only `otpauth://totp` URIs.
The results are held in a one-scan slot (`scan_slot.rs`) under random tokens.
The renderer gets tokens and labels, and sends a token back in a
desktop-only `ItemInputWire`, which `item_input.rs` resolves into the core's
unchanged `ItemInput`.

**Tech Stack:** Rust (Tauri 2), `xcap` 0.9 (screen capture), `rqrr` 0.11
(QR decode, `default-features = false`), `arboard` 3.6 with `image-data`
(clipboard image), `qrcode` 0.14 (already a dependency; used to generate
test images), React + TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-09-27-totp-qr-scan-design.md`

## Global Constraints

- The TOTP secret and the decoded QR text never reach the renderer, a log line, an error message or disk.
- Pixel buffers live only for the duration of one `scan_totp_qr` call.
- Only strings starting with `otpauth://totp` (case-insensitive) that pass `havenkeys_core::totp::parse_totp_input` are kept. A bare Base32 string in a QR code is **not** accepted.
- Clipboard is read first; the screen is captured only when the clipboard gave no TOTP code.
- Pixel budget per decode pass: `MAX_PIXELS = 64_000_000` (all frames together); frames past the budget are skipped.
- Slot: one scan at a time, `SCAN_TTL = 5 minutes`, tokens are 16 random bytes from `havenkeys_core::crypto::fill_random`, as lowercase hex; cleared on lock and after a successful save that used it.
- `havenkeys-core` is not modified.
- Every new `CmdError` is written as a literal `code: "…", message: "…".into()` struct: `src/i18n/errors.test.ts` parses Rust sources with that regex and requires `en.errors.codes` to match word for word.
- Every new command is listed in `build.rs`, `lib.rs` `generate_handler!`, `capabilities/main.json` and `api.ts` (`src/lib/commands.test.ts` enforces it).
- UI strings in both `en.ts` and `pt-BR.ts`.
- Commit messages: conventional (`feat(desktop): …`), **no Co-Authored-By trailer** (user rule).
- Linux build needs `libpipewire-0.3-dev` and `libclang-dev` (decision taken 2026-09-27: accept PipeWire).

## Review Focus

1. **The vault locks while a scan is running** (a Wayland portal dialog can stay open for minutes). Expected: no codes are left in the slot. Pinned in Task 4 (`store_totp_scan` holds the vault guard and refuses when locked) with a slot-level test that `clear` after `replace` leaves nothing.
2. **Saving fails after the token is resolved** (offline, server conflict). Expected: the user can press Save again without scanning again. Pinned in Task 3: `get` does not consume; the slot is cleared only after a successful push (Task 4).
3. **A QR code containing only a Base32 string, or `otpauth://hotp`, or `otpauth-migration://`.** Expected: "The QR code isn't a one-time code setup.", never saved as a TOTP. Pinned in Task 2 tests.
4. **The same QR code visible twice** (two monitors mirroring, or clipboard and screen). Expected: one entry, selected immediately. Pinned in Task 2 (dedupe test).
5. **A clipboard image with transparency, or a malformed buffer** (`rgba.len() != w*h*4`). Expected: transparent reads as white; a malformed frame is skipped, not a panic. Pinned in Task 2 tests.

---

## File Structure

| File | Responsibility |
|---|---|
| `apps/desktop/src-tauri/src/qr_scan.rs` (new) | Frames, decoding, clipboard-then-screen order, OS sources, error mapping |
| `apps/desktop/src-tauri/src/scan_slot.rs` (new) | The one waiting scan: tokens, TTL, lookup, clear |
| `apps/desktop/src-tauri/src/item_input.rs` (new) | `ItemInputWire` / `TotpUpdate` and resolution into core `ItemInput` |
| `apps/desktop/src-tauri/src/state.rs` | `totp_scan` slot in `AppState`; store/resolve/clear helpers; clear on lock |
| `apps/desktop/src-tauri/src/commands.rs` | `scan_totp_qr`; `create_item`/`update_item` take `ItemInputWire` |
| `apps/desktop/src-tauri/{build.rs,src/lib.rs,capabilities/main.json}` | Register and grant the command |
| `apps/desktop/src/lib/secretEdit.ts` (new) | `SecretEdit`, `toUpdate`, `scanLabel`, `canScan` (moved out of the editor, tested) |
| `apps/desktop/src/lib/{types.ts,api.ts}` | `ScannedTotp`, `scanned` update, `scanTotpQr` |
| `apps/desktop/src/views/ItemEditor.tsx` | Button, pick list, scanned preview |
| `apps/desktop/src/components/Icon.tsx`, `src/styles.css`, `src/i18n/{en,pt-BR}.ts` | Icon, styles, strings |
| Build and docs | `Cargo.toml`, `tauri.conf.json`, `.github/workflows/release.yml`, `docs/development.md`, `docs/security-model.md`, `docs/threat-model.md`, the spec |

---

### Task 1: Dependencies and Linux build prerequisites

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/tauri.conf.json` (`bundle`)
- Modify: `.github/workflows/release.yml:41`
- Modify: `docs/development.md:8`
- Modify: `docs/superpowers/specs/2026-09-27-totp-qr-scan-design.md` (§2, §6)

**Interfaces:**
- Produces: crates `xcap`, `rqrr` available to the desktop crate; `arboard::Clipboard::get_image`.

- [ ] **Step 1: Install the system libraries locally (the user runs this; it needs sudo)**

Ask the user to run in the prompt:

```bash
! sudo apt-get install -y libpipewire-0.3-dev libclang-dev
```

Verify: `pkg-config --modversion libpipewire-0.3` prints a version.

- [ ] **Step 2: Add the crates**

In `apps/desktop/src-tauri/Cargo.toml`, replace the `arboard` line and add below `qrcode`:

```toml
# Clipboard is handled in Rust so copied secrets never transit the renderer.
# `image-data` lets the QR scan read a screenshot from the clipboard.
arboard = { version = "3.6", default-features = false, features = ["image-data"] }
```

```toml
# TOTP QR scan (qr_scan.rs): screen capture and a pure-Rust QR decoder. On
# Linux xcap links the system libpipewire-0.3.
xcap = "0.9"
rqrr = { version = "0.11", default-features = false }
```

- [ ] **Step 3: Declare the runtime library for the .deb**

In `apps/desktop/src-tauri/tauri.conf.json`, inside `"bundle"`, after `"shortDescription"`:

```json
    "linux": {
      "deb": {
        "depends": ["libpipewire-0.3-0t64 | libpipewire-0.3-0"]
      }
    },
```

- [ ] **Step 4: CI and docs**

`.github/workflows/release.yml` line 41: append ` libpipewire-0.3-dev libclang-dev` to the `apt-get install` list.

`docs/development.md` line 8: append ` libpipewire-0.3-dev libclang-dev` to the Debian/Ubuntu command, and add after it:

```markdown
    `libpipewire-0.3-dev` and `libclang-dev` are for screen capture
    (the one-time code QR scan); the built app needs `libpipewire-0.3` at
    runtime, which current desktop distributions ship.
```

Spec §2: add a row
`| Linux capture dependency | Accept xcap's libpipewire-0.3 (build: libpipewire-0.3-dev, libclang-dev) | Own X11-only capture; clipboard only on Linux |`.
Spec §6, **Dependencies** bullet: append "On Linux, `xcap` links the system `libpipewire-0.3`; the .deb declares it."

- [ ] **Step 5: Build and audit**

Run: `cd apps/desktop/src-tauri && cargo build -q && cd ../../.. && cargo deny check`
Expected: build succeeds; `advisories ok, bans ok, licenses ok, sources ok`.
If `bans` fails on duplicate versions, read the report: add a `skip` entry to `deny.toml` only for duplicates pulled in by `xcap`, with a comment naming it. If `licenses` fails, stop and report the crate and license to the user; do not widen `allow`.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/Cargo.toml Cargo.lock apps/desktop/src-tauri/tauri.conf.json .github/workflows/release.yml docs/development.md docs/superpowers/specs/2026-09-27-totp-qr-scan-design.md deny.toml
git commit -m "build(desktop): add xcap and rqrr for the TOTP QR scan"
```

---

### Task 2: QR decoding and the clipboard-then-screen order

**Files:**
- Create: `apps/desktop/src-tauri/src/qr_scan.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (add `mod qr_scan;` in the module list, alphabetical)

**Interfaces:**
- Produces:
  - `pub struct Frame { pub width: u32, pub height: u32, pub rgba: Vec<u8> }`
  - `pub struct ScannedCode { pub uri: SecretString, pub issuer: Option<String>, pub account: Option<String> }`
  - `pub enum ScanError { NotFound, NotTotp, Capture }` (Debug, PartialEq, Eq)
  - `pub fn scan(clipboard: impl FnOnce() -> Option<Frame>, screens: impl FnOnce() -> Result<Vec<Frame>, ScanError>) -> Result<Vec<ScannedCode>, ScanError>`
  - `pub fn clipboard_frame() -> Option<Frame>`
  - `pub fn screen_frames() -> Result<Vec<Frame>, ScanError>`
  - `impl From<ScanError> for CmdError`
  - `pub const MAX_PIXELS: u64 = 64_000_000;`

- [ ] **Step 1: Write the failing tests**

Create `apps/desktop/src-tauri/src/qr_scan.rs` with only the test module (plus `mod qr_scan;` in `lib.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const GITHUB: &str =
        "otpauth://totp/GitHub:alice?secret=JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP&issuer=GitHub";
    const GITLAB: &str =
        "otpauth://totp/GitLab:bob?secret=KRUGKIDROVUWG2ZAMJZG653OEBTG66BA&issuer=GitLab";

    /// A QR code for `text`, `scale` pixels per module, with the standard
    /// 4-module quiet zone, black on white.
    fn qr_frame(text: &str, scale: usize) -> Frame {
        let code = qrcode::QrCode::new(text.as_bytes()).unwrap();
        let n = code.width();
        let quiet = 4;
        let size = (n + 2 * quiet) * scale;
        let colors = code.to_colors();
        let mut rgba = vec![255u8; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                let (mx, my) = (x / scale, y / scale);
                let inside = mx >= quiet && my >= quiet && mx < quiet + n && my < quiet + n;
                if inside && colors[(my - quiet) * n + (mx - quiet)] == qrcode::Color::Dark {
                    let i = (y * size + x) * 4;
                    rgba[i..i + 3].fill(0);
                }
            }
        }
        Frame { width: size as u32, height: size as u32, rgba }
    }

    /// Two frames next to each other on one white canvas.
    fn side_by_side(a: &Frame, b: &Frame) -> Frame {
        let width = (a.width + b.width) as usize;
        let height = a.height.max(b.height) as usize;
        let mut rgba = vec![255u8; width * height * 4];
        for (frame, left) in [(a, 0usize), (b, a.width as usize)] {
            let w = frame.width as usize;
            for y in 0..frame.height as usize {
                let src = &frame.rgba[y * w * 4..(y + 1) * w * 4];
                let dst = (y * width + left) * 4;
                rgba[dst..dst + w * 4].copy_from_slice(src);
            }
        }
        Frame { width: width as u32, height: height as u32, rgba }
    }

    fn blank(size: u32) -> Frame {
        Frame { width: size, height: size, rgba: vec![255u8; (size * size * 4) as usize] }
    }

    fn labels(codes: &[ScannedCode]) -> Vec<(Option<&str>, Option<&str>)> {
        codes.iter().map(|c| (c.issuer.as_deref(), c.account.as_deref())).collect()
    }

    #[test]
    fn one_totp_code_is_found_with_its_labels() {
        let found = decode_frames(&[qr_frame(GITHUB, 4)], MAX_PIXELS);
        assert!(found.saw_qr);
        assert_eq!(labels(&found.codes), [(Some("GitHub"), Some("alice"))]);
        assert_eq!(found.codes[0].uri.expose(), GITHUB);
    }

    #[test]
    fn two_codes_are_both_found_and_a_repeat_is_dropped() {
        let two = side_by_side(&qr_frame(GITHUB, 4), &qr_frame(GITLAB, 4));
        let found = decode_frames(&[two], MAX_PIXELS);
        let mut got = labels(&found.codes);
        got.sort();
        assert_eq!(got, [(Some("GitHub"), Some("alice")), (Some("GitLab"), Some("bob"))]);

        let same_twice = side_by_side(&qr_frame(GITHUB, 4), &qr_frame(GITHUB, 4));
        assert_eq!(decode_frames(&[same_twice], MAX_PIXELS).codes.len(), 1);
        let across_frames = [qr_frame(GITHUB, 4), qr_frame(GITHUB, 3)];
        assert_eq!(decode_frames(&across_frames, MAX_PIXELS).codes.len(), 1);
    }

    #[test]
    fn qr_codes_that_are_not_a_totp_setup_are_seen_but_not_kept() {
        for text in [
            "https://github.com/settings/security",
            "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP",
            "otpauth://hotp/GitHub:alice?secret=JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP&counter=1",
            "otpauth-migration://offline?data=CjEKCkhlbGxvId6tvu8SGEV4YW1wbGU",
        ] {
            let found = decode_frames(&[qr_frame(text, 4)], MAX_PIXELS);
            assert!(found.saw_qr, "{text}");
            assert!(found.codes.is_empty(), "{text}");
        }
    }

    #[test]
    fn a_blank_image_has_no_qr_code() {
        let found = decode_frames(&[blank(200)], MAX_PIXELS);
        assert!(!found.saw_qr);
        assert!(found.codes.is_empty());
    }

    #[test]
    fn frames_past_the_pixel_budget_or_malformed_are_skipped() {
        let qr = qr_frame(GITHUB, 4);
        let pixels = u64::from(qr.width) * u64::from(qr.height);
        assert!(decode_frames(&[qr_frame(GITHUB, 4)], pixels - 1).codes.is_empty());
        assert_eq!(decode_frames(&[qr_frame(GITHUB, 4)], pixels).codes.len(), 1);

        let mut short = qr_frame(GITHUB, 4);
        short.rgba.truncate(short.rgba.len() - 1);
        assert!(decode_frames(&[short], MAX_PIXELS).codes.is_empty());
        let empty = Frame { width: 0, height: 0, rgba: vec![] };
        assert!(decode_frames(&[empty], MAX_PIXELS).codes.is_empty());
    }

    #[test]
    fn transparent_pixels_read_as_white() {
        let mut qr = qr_frame(GITHUB, 4);
        // Make the white background fully transparent black, as a pasted PNG
        // with alpha can arrive.
        for px in qr.rgba.chunks_exact_mut(4) {
            if px[0] == 255 {
                px.copy_from_slice(&[0, 0, 0, 0]);
            }
        }
        assert_eq!(decode_frames(&[qr], MAX_PIXELS).codes.len(), 1);
    }

    #[test]
    fn a_totp_code_on_the_clipboard_means_the_screen_is_never_captured() {
        let captured = Cell::new(false);
        let codes = scan(
            || Some(qr_frame(GITHUB, 4)),
            || {
                captured.set(true);
                Ok(vec![qr_frame(GITLAB, 4)])
            },
        )
        .unwrap();
        assert!(!captured.get());
        assert_eq!(labels(&codes), [(Some("GitHub"), Some("alice"))]);
    }

    #[test]
    fn without_a_totp_code_on_the_clipboard_the_screen_is_used() {
        let from_screen = || Ok(vec![blank(50), qr_frame(GITLAB, 4)]);
        let bob = [(Some("GitLab"), Some("bob"))];
        assert_eq!(labels(&scan(|| None, from_screen).unwrap()), bob);
        assert_eq!(labels(&scan(|| Some(blank(100)), from_screen).unwrap()), bob);
        let url = || Some(qr_frame("https://example.com", 4));
        assert_eq!(labels(&scan(url, from_screen).unwrap()), bob);
    }

    #[test]
    fn scan_errors_say_what_was_seen() {
        let nothing = || Ok(vec![blank(100)]);
        let url = || Some(qr_frame("https://example.com", 4));
        assert_eq!(scan(|| None, nothing).err(), Some(ScanError::NotFound));
        assert_eq!(scan(url, nothing).err(), Some(ScanError::NotTotp));
        let url_on_screen = || Ok(vec![qr_frame("https://example.com", 4)]);
        assert_eq!(scan(|| None, url_on_screen).err(), Some(ScanError::NotTotp));
        assert_eq!(
            scan(|| None, || Err(ScanError::Capture)).err(),
            Some(ScanError::Capture)
        );
    }

    #[test]
    fn scan_errors_never_carry_decoded_text() {
        for e in [ScanError::NotFound, ScanError::NotTotp, ScanError::Capture] {
            let err = CmdError::from(e);
            assert!(!err.message.contains("otpauth"));
            assert!(!err.message.contains("JBSWY3DP"));
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd apps/desktop/src-tauri && cargo test -q qr_scan`
Expected: compile errors — `Frame`, `decode_frames`, `scan`, `ScanError`, `MAX_PIXELS` not found.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `qr_scan.rs`:

```rust
//! Reads a TOTP setup QR code from the clipboard image or, failing that, the
//! screen (docs/superpowers/specs/2026-09-27-totp-qr-scan-design.md).
//!
//! Pixels and decoded text live only in memory for the length of one scan.
//! Only strings that parse as `otpauth://totp` survive; anything else a QR
//! code on screen might say is dropped without being logged or returned.

use crate::state::CmdError;
use havenkeys_core::totp::{parse_totp_input, TotpAlgorithm};
use havenkeys_core::SecretString;
use zeroize::Zeroizing;

/// Upper bound on the pixels decoded in one pass, all frames together:
/// three 4K monitors are about 25 million.
pub const MAX_PIXELS: u64 = 64_000_000;

/// One RGBA image, 4 bytes per pixel, rows top to bottom.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A TOTP setup found in a QR code. `uri` is the full `otpauth://` text.
pub struct ScannedCode {
    pub uri: SecretString,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScanError {
    /// No QR code in the clipboard image, if any, or on screen.
    NotFound,
    /// QR codes were found, but none is a TOTP setup.
    NotTotp,
    /// The screen could not be captured (no display, portal refused).
    Capture,
}

impl From<ScanError> for CmdError {
    fn from(e: ScanError) -> Self {
        match e {
            // macOS returns only the wallpaper when Screen Recording is
            // denied, so "nothing found" may really be "not allowed".
            ScanError::NotFound if cfg!(target_os = "macos") => CmdError {
                code: "qr_not_found_macos",
                message: "No QR code found. Make sure it's fully visible on screen. If this is the first scan, allow HavenKeys in System Settings → Privacy & Security → Screen Recording.".into(),
            },
            ScanError::NotFound => CmdError {
                code: "qr_not_found",
                message: "No QR code found. Make sure it's fully visible on screen.".into(),
            },
            ScanError::NotTotp => CmdError {
                code: "qr_not_totp",
                message: "The QR code isn't a one-time code setup.".into(),
            },
            ScanError::Capture => CmdError {
                code: "screen_capture",
                message: "Could not capture the screen.".into(),
            },
        }
    }
}

#[derive(Default)]
struct Decoded {
    codes: Vec<ScannedCode>,
    /// Some QR code was decoded, TOTP or not.
    saw_qr: bool,
}

/// Grey level of one RGBA pixel, composited over white so a transparent
/// background reads as paper rather than ink.
fn luma(px: &[u8]) -> u8 {
    let [r, g, b, a] = [px[0], px[1], px[2], px[3]].map(u32::from);
    let y = (r * 299 + g * 587 + b * 114) / 1000;
    ((y * a + 255 * (255 - a)) / 255) as u8
}

/// Every TOTP setup in `frames`, without repeats. Frames that are malformed
/// or would take the total past `max_pixels` are skipped.
fn decode_frames(frames: &[Frame], max_pixels: u64) -> Decoded {
    let mut out = Decoded::default();
    let mut seen: Vec<(Zeroizing<String>, TotpAlgorithm, u32, u32)> = Vec::new();
    let mut budget = max_pixels;
    for frame in frames {
        let (w, h) = (frame.width as usize, frame.height as usize);
        let pixels = u64::from(frame.width) * u64::from(frame.height);
        if w == 0 || h == 0 || frame.rgba.len() != w * h * 4 || pixels > budget {
            continue;
        }
        budget -= pixels;
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
            let i = (y * w + x) * 4;
            luma(&frame.rgba[i..i + 4])
        });
        for grid in image.detect_grids() {
            let Ok((_, text)) = grid.decode() else {
                continue;
            };
            out.saw_qr = true;
            let mut text = Zeroizing::new(text);
            let is_totp_uri = text
                .get(.."otpauth://totp".len())
                .is_some_and(|p| p.eq_ignore_ascii_case("otpauth://totp"));
            if !is_totp_uri {
                continue;
            }
            let Ok(config) = parse_totp_input(&text) else {
                continue;
            };
            let key = (
                Zeroizing::new(config.secret.expose().to_owned()),
                config.algorithm,
                config.digits,
                config.period,
            );
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            out.codes.push(ScannedCode {
                uri: SecretString::new(std::mem::take(&mut *text)),
                issuer: config.issuer.clone(),
                account: config.account.clone(),
            });
        }
    }
    out
}

/// The clipboard image first; the screen only if that gave no TOTP code.
/// Both sources are passed in so the order is testable without a display.
pub fn scan(
    clipboard: impl FnOnce() -> Option<Frame>,
    screens: impl FnOnce() -> Result<Vec<Frame>, ScanError>,
) -> Result<Vec<ScannedCode>, ScanError> {
    let mut saw_qr = false;
    if let Some(frame) = clipboard() {
        let found = decode_frames(std::slice::from_ref(&frame), MAX_PIXELS);
        drop(frame);
        if !found.codes.is_empty() {
            return Ok(found.codes);
        }
        saw_qr = found.saw_qr;
    }
    let frames = screens()?;
    let found = decode_frames(&frames, MAX_PIXELS);
    drop(frames);
    if !found.codes.is_empty() {
        Ok(found.codes)
    } else if saw_qr || found.saw_qr {
        Err(ScanError::NotTotp)
    } else {
        Err(ScanError::NotFound)
    }
}

/// The image on the clipboard, if there is one. Only read, never changed; an
/// unreadable clipboard counts as "no image" and the scan moves on.
pub fn clipboard_frame() -> Option<Frame> {
    let image = arboard::Clipboard::new().ok()?.get_image().ok()?;
    Some(Frame {
        width: u32::try_from(image.width).ok()?,
        height: u32::try_from(image.height).ok()?,
        rgba: image.bytes.into_owned(),
    })
}

/// One capture per monitor. A monitor that fails is skipped; none at all is
/// an error.
pub fn screen_frames() -> Result<Vec<Frame>, ScanError> {
    let monitors = xcap::Monitor::all().map_err(|_| ScanError::Capture)?;
    let frames: Vec<Frame> = monitors
        .iter()
        .filter_map(|m| m.capture_image().ok())
        .map(|image| Frame {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        })
        .collect();
    if frames.is_empty() {
        Err(ScanError::Capture)
    } else {
        Ok(frames)
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd apps/desktop/src-tauri && cargo test -q qr_scan`
Expected: 10 passed. `clipboard_frame` / `screen_frames` are unused until Task 4; a `dead_code` warning is expected here and goes away there.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/qr_scan.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): decode TOTP setup QR codes, clipboard first then screen"
```

---

### Task 3: The scan slot and the edit input that uses it

**Files:**
- Create: `apps/desktop/src-tauri/src/scan_slot.rs`
- Create: `apps/desktop/src-tauri/src/item_input.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (add `mod item_input;` and `mod scan_slot;`)

**Interfaces:**
- Consumes: `qr_scan::ScannedCode` (Task 2).
- Produces:
  - `scan_slot::SCAN_TTL: Duration`
  - `scan_slot::ScannedTotp { pub token: String, pub issuer: Option<String>, pub account: Option<String> }` (Serialize, camelCase)
  - `scan_slot::ScanSlot` (Default) with `replace(&mut self, Vec<ScannedCode>, Instant) -> havenkeys_core::Result<Vec<ScannedTotp>>`, `get(&self, &str, Instant) -> Option<SecretString>`, `clear(&mut self)`
  - `item_input::TotpUpdate { Keep, Set(SecretString), Clear, Scanned(String) }`
  - `item_input::ItemInputWire` (Deserialize) with `uses_scan(&self) -> bool` and `resolve(self, &ScanSlot, Instant) -> CmdResult<ItemInput>`

- [ ] **Step 1: Write the failing slot tests**

Create `apps/desktop/src-tauri/src/scan_slot.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn code(issuer: &str, uri: &str) -> ScannedCode {
        ScannedCode {
            uri: SecretString::new(uri.to_owned()),
            issuer: Some(issuer.to_owned()),
            account: None,
        }
    }

    #[test]
    fn a_token_finds_its_code_until_cleared() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        let out = slot.replace(vec![code("A", "uri-a"), code("B", "uri-b")], t0).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].issuer.as_deref(), Some("B"));
        assert_eq!(out[0].token.len(), 32);
        assert_ne!(out[0].token, out[1].token);
        // Looking a token up does not use it: a failed save can be retried.
        assert_eq!(slot.get(&out[1].token, t0).unwrap().expose(), "uri-b");
        assert_eq!(slot.get(&out[1].token, t0).unwrap().expose(), "uri-b");
        slot.clear();
        assert!(slot.get(&out[1].token, t0).is_none());
    }

    #[test]
    fn unknown_and_expired_tokens_find_nothing() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        assert!(slot.get("00", t0).is_none());
        let out = slot.replace(vec![code("A", "uri-a")], t0).unwrap();
        assert!(slot.get("not-a-token", t0).is_none());
        assert!(slot.get(&out[0].token, t0 + SCAN_TTL - Duration::from_secs(1)).is_some());
        assert!(slot.get(&out[0].token, t0 + SCAN_TTL).is_none());
    }

    #[test]
    fn a_new_scan_replaces_the_old_one() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        let old = slot.replace(vec![code("A", "uri-a")], t0).unwrap();
        let new = slot.replace(vec![code("B", "uri-b")], t0).unwrap();
        assert!(slot.get(&old[0].token, t0).is_none());
        assert_eq!(slot.get(&new[0].token, t0).unwrap().expose(), "uri-b");
    }

    #[test]
    fn the_preview_never_carries_the_uri() {
        let mut slot = ScanSlot::default();
        let out = slot.replace(vec![code("A", "otpauth://totp/x?secret=S")], Instant::now()).unwrap();
        let json = serde_json::to_string(&out).unwrap();
        assert!(!json.contains("otpauth"));
        assert!(json.contains("\"token\""));
        assert!(json.contains("\"issuer\":\"A\""));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd apps/desktop/src-tauri && cargo test -q scan_slot`
Expected: compile errors — `ScanSlot`, `SCAN_TTL` not found.

- [ ] **Step 3: Implement the slot**

Above the tests in `scan_slot.rs`:

```rust
//! The one QR scan whose codes are waiting to be saved (design §4.2). The
//! renderer only ever holds a token and the labels; the `otpauth://` URI
//! stays here until an item is saved with it, the vault locks, a new scan
//! replaces it, or five minutes pass.

use crate::qr_scan::ScannedCode;
use havenkeys_core::crypto::fill_random;
use havenkeys_core::SecretString;
use serde::Serialize;
use std::time::{Duration, Instant};

pub const SCAN_TTL: Duration = Duration::from_secs(5 * 60);

/// What the renderer gets for each code found: never the URI.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedTotp {
    pub token: String,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

#[derive(Default)]
pub struct ScanSlot {
    batch: Option<Batch>,
}

struct Batch {
    created: Instant,
    entries: Vec<(String, SecretString)>,
}

impl ScanSlot {
    /// Forget the previous scan and hold `codes`, each under a fresh
    /// 128-bit token.
    pub fn replace(
        &mut self,
        codes: Vec<ScannedCode>,
        now: Instant,
    ) -> havenkeys_core::Result<Vec<ScannedTotp>> {
        self.batch = None;
        let mut entries = Vec::with_capacity(codes.len());
        let mut previews = Vec::with_capacity(codes.len());
        for code in codes {
            let mut raw = [0u8; 16];
            fill_random(&mut raw)?;
            let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
            previews.push(ScannedTotp {
                token: token.clone(),
                issuer: code.issuer,
                account: code.account,
            });
            entries.push((token, code.uri));
        }
        self.batch = Some(Batch { created: now, entries });
        Ok(previews)
    }

    /// The URI for `token` while the scan is fresh. Looking it up does not
    /// use it up, so a save that fails can be retried; `clear` after a
    /// successful save.
    pub fn get(&self, token: &str, now: Instant) -> Option<SecretString> {
        let batch = self.batch.as_ref()?;
        if now.saturating_duration_since(batch.created) >= SCAN_TTL {
            return None;
        }
        batch
            .entries
            .iter()
            .find(|(t, _)| t == token)
            .map(|(_, uri)| uri.clone())
    }

    pub fn clear(&mut self) {
        self.batch = None;
    }
}
```

- [ ] **Step 4: Run the slot tests**

Run: `cd apps/desktop/src-tauri && cargo test -q scan_slot`
Expected: 4 passed.

- [ ] **Step 5: Write the failing edit-input tests**

Create `apps/desktop/src-tauri/src/item_input.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::qr_scan::ScannedCode;
    use serde_json::json;

    fn wire(totp: serde_json::Value) -> ItemInputWire {
        serde_json::from_value(json!({
            "itemType": "login",
            "title": "GitHub",
            "username": "alice",
            "urls": [{ "url": "https://github.com", "matchType": "domain" }],
            "password": { "op": "keep" },
            "totp": totp,
            "autoSignIn": false
        }))
        .unwrap()
    }

    fn slot_with(uri: &str, now: Instant) -> (ScanSlot, String) {
        let mut slot = ScanSlot::default();
        let token = slot
            .replace(
                vec![ScannedCode { uri: SecretString::new(uri.into()), issuer: None, account: None }],
                now,
            )
            .unwrap()
            .remove(0)
            .token;
        (slot, token)
    }

    #[test]
    fn a_scanned_token_becomes_a_set_with_the_scanned_uri() {
        let now = Instant::now();
        let (slot, token) = slot_with("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP", now);
        let input = wire(json!({ "op": "scanned", "value": token }));
        assert!(input.uses_scan());
        let input = input.resolve(&slot, now).unwrap();
        match input.totp {
            SecretUpdate::Set(v) => assert_eq!(v.expose(), "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP"),
            _ => panic!("expected Set"),
        }
        assert_eq!(input.title, "GitHub");
        assert_eq!(input.username.as_deref(), Some("alice"));
        assert_eq!(input.urls.len(), 1);
        assert!(matches!(input.password, SecretUpdate::Keep));
        assert_eq!(input.auto_sign_in, Some(false));
    }

    #[test]
    fn an_unknown_or_expired_token_is_refused() {
        let now = Instant::now();
        let (slot, token) = slot_with("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP", now);
        let err = wire(json!({ "op": "scanned", "value": "feed" })).resolve(&slot, now).err().unwrap();
        assert_eq!(err.code, "scan_expired");
        let late = now + crate::scan_slot::SCAN_TTL;
        assert!(wire(json!({ "op": "scanned", "value": token })).resolve(&slot, late).is_err());
    }

    #[test]
    fn the_other_totp_updates_pass_through() {
        let slot = ScanSlot::default();
        let now = Instant::now();
        let keep = wire(json!({ "op": "keep" }));
        assert!(!keep.uses_scan());
        assert!(matches!(keep.resolve(&slot, now).unwrap().totp, SecretUpdate::Keep));
        assert!(matches!(wire(json!({ "op": "clear" })).resolve(&slot, now).unwrap().totp, SecretUpdate::Clear));
        match wire(json!({ "op": "set", "value": "JBSWY3DPEHPK3PXP" })).resolve(&slot, now).unwrap().totp {
            SecretUpdate::Set(v) => assert_eq!(v.expose(), "JBSWY3DPEHPK3PXP"),
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn unknown_fields_are_still_refused() {
        let bad = serde_json::from_value::<ItemInputWire>(json!({
            "itemType": "login", "title": "x", "extra": 1
        }));
        assert!(bad.is_err());
    }
}
```

- [ ] **Step 6: Run to verify they fail**

Run: `cd apps/desktop/src-tauri && cargo test -q item_input`
Expected: compile errors — `ItemInputWire` not found.

- [ ] **Step 7: Implement the edit input**

Above the tests in `item_input.rs`:

```rust
//! The item create/update request as the desktop renderer sends it. The same
//! as the core's `ItemInput`, except that `totp` may also name a scanned QR
//! code by token (design §4.4); the URI is looked up here, in Rust, so the
//! core sees an ordinary `Set` and the renderer never saw the secret.

use crate::scan_slot::ScanSlot;
use crate::state::{CmdError, CmdResult};
use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate, UrlRule};
use havenkeys_core::SecretString;
use serde::Deserialize;
use std::time::Instant;

#[derive(Default, Deserialize)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum TotpUpdate {
    #[default]
    Keep,
    Set(SecretString),
    Clear,
    /// A token from `scan_totp_qr`.
    Scanned(String),
}

/// Mirrors `havenkeys_core::model::ItemInput` field for field; a field the
/// core gains must be added here too (`deny_unknown_fields` makes a UI that
/// sends it fail loudly rather than lose it).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemInputWire {
    pub item_type: ItemType,
    pub title: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub urls: Vec<UrlRule>,
    #[serde(default)]
    pub password: SecretUpdate,
    #[serde(default)]
    pub totp: TotpUpdate,
    #[serde(default)]
    pub notes: SecretUpdate,
    #[serde(default)]
    pub content: SecretUpdate,
    #[serde(default)]
    pub auto_sign_in: Option<bool>,
}

fn scan_expired() -> CmdError {
    CmdError {
        code: "scan_expired",
        message: "The scanned code expired. Scan it again.".into(),
    }
}

impl ItemInputWire {
    /// Whether saving this uses a scanned code, so the caller can empty the
    /// slot once the save has gone through.
    pub fn uses_scan(&self) -> bool {
        matches!(self.totp, TotpUpdate::Scanned(_))
    }

    pub fn resolve(self, slot: &ScanSlot, now: Instant) -> CmdResult<ItemInput> {
        let totp = match self.totp {
            TotpUpdate::Keep => SecretUpdate::Keep,
            TotpUpdate::Set(value) => SecretUpdate::Set(value),
            TotpUpdate::Clear => SecretUpdate::Clear,
            TotpUpdate::Scanned(token) => {
                SecretUpdate::Set(slot.get(&token, now).ok_or_else(scan_expired)?)
            }
        };
        Ok(ItemInput {
            item_type: self.item_type,
            title: self.title,
            username: self.username,
            urls: self.urls,
            password: self.password,
            totp,
            notes: self.notes,
            content: self.content,
            auto_sign_in: self.auto_sign_in,
        })
    }
}
```

- [ ] **Step 8: Run all three modules' tests and clippy**

Run: `cd apps/desktop/src-tauri && cargo test -q && cargo clippy -q --all-targets`
Expected: all tests pass (item_input: 4, scan_slot: 4, qr_scan: 10, plus the existing ones); clippy shows only `dead_code` warnings for items first used in Task 4.

- [ ] **Step 9: Commit**

```bash
git add apps/desktop/src-tauri/src/scan_slot.rs apps/desktop/src-tauri/src/item_input.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): hold scanned TOTP codes in Rust behind one-time tokens"
```

---

### Task 4: The `scan_totp_qr` command and saving with a token

**Files:**
- Modify: `apps/desktop/src-tauri/src/state.rs` (field, constructor, helpers, `lock`)
- Modify: `apps/desktop/src-tauri/src/commands.rs` (`scan_totp_qr`, `create_item`, `update_item`, imports)
- Modify: `apps/desktop/src-tauri/build.rs`, `apps/desktop/src-tauri/src/lib.rs`, `apps/desktop/src-tauri/capabilities/main.json`
- Modify: `apps/desktop/src/lib/types.ts`, `apps/desktop/src/lib/api.ts`
- Modify: `apps/desktop/src/i18n/en.ts`, `apps/desktop/src/i18n/pt-BR.ts` (error codes)
- Test: `apps/desktop/src/lib/commands.test.ts`, `apps/desktop/src/i18n/errors.test.ts` (existing; they now cover the new command and codes)

**Interfaces:**
- Consumes: `qr_scan::{scan, clipboard_frame, screen_frames}`, `scan_slot::{ScanSlot, ScannedTotp}`, `item_input::ItemInputWire`.
- Produces:
  - Tauri command `scan_totp_qr` → `ScannedTotp[]`
  - `create_item` / `update_item` accept `totp: { op: "scanned", value: token }`
  - TS: `interface ScannedTotp { token: string; issuer: string | null; account: string | null }`, `SecretUpdate` gains `{ op: "scanned"; value: string }`, `api.scanTotpQr(): Promise<ScannedTotp[]>`
  - Error codes `qr_not_found`, `qr_not_found_macos`, `qr_not_totp`, `screen_capture`, `scan_expired`

- [ ] **Step 1: Make the frontend tests fail first**

`apps/desktop/src/lib/api.ts`, after `openWebsite`:

```ts
  /** Rust reads the clipboard image, then the screen; returns tokens and labels, never the secret. */
  scanTotpQr: () => call<ScannedTotp[]>("scan_totp_qr"),
```

and add `ScannedTotp` to the `import type { … } from "./types"` list at the top of `api.ts`.

`apps/desktop/src/lib/types.ts`:

```ts
export type SecretUpdate =
  | { op: "keep" }
  | { op: "set"; value: string }
  | { op: "clear" }
  /** A token from `scan_totp_qr`; TOTP only. */
  | { op: "scanned"; value: string };
```

and after `TotpCode`'s interface:

```ts
/** One code a QR scan found. The secret stays in Rust under `token`. */
export interface ScannedTotp {
  token: string;
  issuer: string | null;
  account: string | null;
}
```

Run: `cd apps/desktop && pnpm -s test`
Expected: FAIL in `commands.test.ts` — `scan_totp_qr` is in `api.ts` but not in `build.rs` / `lib.rs` / `main.json`.

- [ ] **Step 2: AppState — slot, helpers, clear on lock**

In `apps/desktop/src-tauri/src/state.rs`:

Imports: add

```rust
use crate::item_input::ItemInputWire;
use crate::qr_scan::ScannedCode;
use crate::scan_slot::{ScanSlot, ScannedTotp};
use havenkeys_core::model::ItemInput;
```

Field, after `last_import`:

```rust
    /// The last QR scan's codes, waiting to be saved (scan_slot.rs).
    totp_scan: Mutex<ScanSlot>,
```

Constructor, after `last_import: Mutex::new(None),`:

```rust
            totp_scan: Mutex::new(ScanSlot::default()),
```

Methods, in `impl AppState` after `data_dir()`:

```rust
    /// Hold a scan's codes. The vault guard is held throughout so this is
    /// ordered against `lock()`, which clears the slot after taking that
    /// guard: a scan that finishes as the vault locks leaves nothing behind.
    pub fn store_totp_scan(&self, codes: Vec<ScannedCode>) -> CmdResult<Vec<ScannedTotp>> {
        let vault = self.vault()?;
        if !vault.is_unlocked() {
            return Err(havenkeys_core::Error::Locked.into());
        }
        let mut slot = self.totp_scan.lock().map_err(|_| CmdError::internal())?;
        Ok(slot.replace(codes, Instant::now())?)
    }

    /// The core's `ItemInput` for a renderer request, with a scanned token
    /// replaced by its URI.
    pub fn resolve_item_input(&self, input: ItemInputWire) -> CmdResult<ItemInput> {
        let slot = self.totp_scan.lock().map_err(|_| CmdError::internal())?;
        input.resolve(&slot, Instant::now())
    }

    pub fn clear_totp_scan(&self) {
        if let Ok(mut slot) = self.totp_scan.lock() {
            slot.clear();
        }
    }
```

In `lock()`, right after the `last_import` block:

```rust
        // Scanned TOTP codes waiting to be saved are vault secrets too.
        self.clear_totp_scan();
```

- [ ] **Step 3: The command and the save path**

In `apps/desktop/src-tauri/src/commands.rs`:

Imports: add `use crate::item_input::ItemInputWire;`, `use crate::qr_scan;`, `use crate::scan_slot::ScannedTotp;`; remove `ItemInput` from the `havenkeys_core::model::{…}` import if nothing else uses it (`cargo clippy` will say).

Add after `open_website`:

```rust
/// Look for a TOTP setup QR code: the clipboard image first, then every
/// monitor. Returns a token and labels per code; the URI stays in Rust
/// (scan_slot.rs) until an item is saved with the token.
#[tauri::command]
pub async fn scan_totp_qr(app: AppHandle) -> CmdResult<Vec<ScannedTotp>> {
    {
        let state = app.state::<AppState>();
        state.touch();
        if !state.vault()?.is_unlocked() {
            return Err(havenkeys_core::Error::Locked.into());
        }
    }
    // Capture and decoding take a moment, and a Wayland portal waits for
    // the user: keep them off the main thread.
    let codes = tauri::async_runtime::spawn_blocking(|| {
        qr_scan::scan(qr_scan::clipboard_frame, qr_scan::screen_frames)
    })
    .await
    .map_err(|_| CmdError::internal())??;
    app.state::<AppState>().store_totp_scan(codes)
}
```

(`??`: the first `?` is the join error mapped above, the second converts `ScanError` through `From<ScanError> for CmdError`.)

Replace `create_item`:

```rust
#[tauri::command]
pub async fn create_item(app: AppHandle, input: ItemInputWire) -> CmdResult<ItemOverview> {
    let uses_scan = input.uses_scan();
    let staged = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_online()?;
        let input = state.resolve_item_input(input)?;
        let staged = state.vault()?.stage_create(input, AppState::now_ms())?;
        staged
    };
    let saved = sync::push(&app, staged)
        .await?
        .ok_or_else(CmdError::internal)?;
    if uses_scan {
        app.state::<AppState>().clear_totp_scan();
    }
    Ok(saved)
}
```

Replace `update_item` the same way:

```rust
#[tauri::command]
pub async fn update_item(app: AppHandle, id: Uuid, input: ItemInputWire) -> CmdResult<ItemOverview> {
    let uses_scan = input.uses_scan();
    let staged = {
        let state = app.state::<AppState>();
        state.touch();
        state.require_online()?;
        let input = state.resolve_item_input(input)?;
        let staged = state
            .vault()?
            .stage_update(&id, input, AppState::now_ms())?;
        staged
    };
    let saved = sync::push(&app, staged)
        .await?
        .ok_or_else(CmdError::internal)?;
    if uses_scan {
        app.state::<AppState>().clear_totp_scan();
    }
    Ok(saved)
}
```

Keep the doc comment above `create_item` as it is.

- [ ] **Step 4: Register and grant the command**

`build.rs`: add `"scan_totp_qr",` after `"open_website",`.
`src/lib.rs` `generate_handler!`: add `commands::scan_totp_qr,` after `commands::open_website,`.
`capabilities/main.json`: add `"allow-scan-totp-qr",` after `"allow-open-website",`.

- [ ] **Step 5: Error strings**

`apps/desktop/src/i18n/en.ts` — `ErrorCode` union, after `| "open_website"`:

```ts
  | "qr_not_found"
  | "qr_not_found_macos"
  | "qr_not_totp"
  | "screen_capture"
  | "scan_expired"
```

`codes`, after `open_website: …`:

```ts
  qr_not_found: "No QR code found. Make sure it's fully visible on screen.",
  qr_not_found_macos:
    "No QR code found. Make sure it's fully visible on screen. If this is the first scan, allow HavenKeys in System Settings → Privacy & Security → Screen Recording.",
  qr_not_totp: "The QR code isn't a one-time code setup.",
  screen_capture: "Could not capture the screen.",
  scan_expired: "The scanned code expired. Scan it again.",
```

`apps/desktop/src/i18n/pt-BR.ts` `codes`, after `open_website: …`:

```ts
  qr_not_found: "Nenhum QR code encontrado. Verifique se ele está inteiro na tela.",
  qr_not_found_macos:
    "Nenhum QR code encontrado. Verifique se ele está inteiro na tela. Se esta é a primeira leitura, permita o HavenKeys em Ajustes do Sistema → Privacidade e Segurança → Gravação de Tela.",
  qr_not_totp: "O QR code não é de configuração de códigos de verificação.",
  screen_capture: "Não foi possível capturar a tela.",
  scan_expired: "O código lido expirou. Leia o QR code de novo.",
```

- [ ] **Step 6: Run everything**

Run: `cd apps/desktop/src-tauri && cargo clippy -q --all-targets && cargo test -q && cd .. && pnpm -s typecheck && pnpm -s test`
Expected: clippy clean (no more `dead_code`); Rust tests pass; typecheck OK; vitest passes, including `commands.test.ts` and the error-table tests in `errors.test.ts`.

Note: `pnpm typecheck` fails at this point if `ItemEditor.tsx`'s local `toUpdate` no longer covers `SecretUpdate` — it does not return `scanned`, so it still type-checks. If it doesn't, fix it in Task 5, not here.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src-tauri apps/desktop/src/lib apps/desktop/src/i18n
git commit -m "feat(desktop): scan_totp_qr command; save a scanned code by token"
```

---

### Task 5: The editor button, pick list and scanned preview

**Files:**
- Create: `apps/desktop/src/lib/secretEdit.ts`, `apps/desktop/src/lib/secretEdit.test.ts`
- Modify: `apps/desktop/src/views/ItemEditor.tsx` (lines 19-26 move out; TOTP row ~321-356)
- Modify: `apps/desktop/src/components/Icon.tsx` (new `qr` path)
- Modify: `apps/desktop/src/styles.css`
- Modify: `apps/desktop/src/i18n/en.ts`, `apps/desktop/src/i18n/pt-BR.ts` (`editor` strings)

**Interfaces:**
- Consumes: `api.scanTotpQr()`, `ScannedTotp`, `SecretUpdate` with `scanned` (Task 4).
- Produces: `secretEdit.ts` exports `SecretEdit`, `KEEP`, `EMPTY`, `toUpdate`, `scanLabel`, `canScan`.

- [ ] **Step 1: Write the failing helper tests**

`apps/desktop/src/lib/secretEdit.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { EMPTY, KEEP, canScan, scanLabel, toUpdate } from "./secretEdit";

describe("toUpdate", () => {
  it("maps each edit to what Rust expects", () => {
    expect(toUpdate(KEEP)).toEqual({ op: "keep" });
    expect(toUpdate({ mode: "clear" })).toEqual({ op: "clear" });
    expect(toUpdate({ mode: "set", value: "abc" })).toEqual({ op: "set", value: "abc" });
    expect(toUpdate(EMPTY)).toEqual({ op: "clear" });
    expect(toUpdate({ mode: "scanned", token: "t0k", label: "GitHub" })).toEqual({ op: "scanned", value: "t0k" });
  });
});

describe("scanLabel", () => {
  it("joins issuer and account, skipping what is missing", () => {
    expect(scanLabel({ token: "t", issuer: "GitHub", account: "alice" }, "Unnamed")).toBe("GitHub · alice");
    expect(scanLabel({ token: "t", issuer: null, account: "alice" }, "Unnamed")).toBe("alice");
    expect(scanLabel({ token: "t", issuer: " ", account: null }, "Unnamed")).toBe("Unnamed");
  });
});

describe("canScan", () => {
  it("offers the QR button only while the field is empty", () => {
    expect(canScan(EMPTY)).toBe(true);
    expect(canScan({ mode: "set", value: "JBSW" })).toBe(false);
    expect(canScan(KEEP)).toBe(false);
    expect(canScan({ mode: "clear" })).toBe(false);
    expect(canScan({ mode: "scanned", token: "t", label: "x" })).toBe(false);
  });
});
```

Run: `cd apps/desktop && pnpm -s test secretEdit`
Expected: FAIL — cannot resolve `./secretEdit`.

- [ ] **Step 2: Implement the helpers**

`apps/desktop/src/lib/secretEdit.ts`:

```ts
import type { ScannedTotp, SecretUpdate } from "./types";

/**
 * How the editor holds a secret field. `scanned` is TOTP only: the secret
 * stays in Rust and the editor keeps the token and a label to show.
 */
export type SecretEdit =
  | { mode: "keep" }
  | { mode: "clear" }
  | { mode: "set"; value: string }
  | { mode: "scanned"; token: string; label: string };

export const KEEP: SecretEdit = { mode: "keep" };
export const EMPTY: SecretEdit = { mode: "set", value: "" };

export function toUpdate(edit: SecretEdit): SecretUpdate {
  if (edit.mode === "set") return edit.value ? { op: "set", value: edit.value } : { op: "clear" };
  if (edit.mode === "scanned") return { op: "scanned", value: edit.token };
  return { op: edit.mode };
}

/** "Issuer · account", or `unnamed` when the QR code carried neither. */
export function scanLabel(code: ScannedTotp, unnamed: string): string {
  const parts = [code.issuer, code.account]
    .map((s) => s?.trim())
    .filter((s): s is string => !!s);
  return parts.length > 0 ? parts.join(" · ") : unnamed;
}

/** The QR button belongs in the row while the field is empty. */
export function canScan(edit: SecretEdit): boolean {
  return edit.mode === "set" && edit.value === "";
}
```

In `ItemEditor.tsx`, delete the local `type SecretEdit`, `const KEEP` and `function toUpdate` (lines 19-26) and import them:

```ts
import { EMPTY, KEEP, canScan, scanLabel, toUpdate, type SecretEdit } from "../lib/secretEdit";
import type { ScannedTotp } from "../lib/types";
```

(merge `ScannedTotp` into the existing `import type { … } from "../lib/types"` line if there is one). Replace the literal `{ mode: "set", value: "" }` for `password` and `totp` initial state and the TOTP **Replace** button with `EMPTY`.

Run: `pnpm -s test secretEdit && pnpm -s typecheck`
Expected: 3 passed; typecheck OK.

- [ ] **Step 3: Icon and strings**

`apps/desktop/src/components/Icon.tsx`, add to `paths` after `grid`:

```ts
  qr: "M4.5 4.5h6v6h-6zM13.5 4.5h6v6h-6zM4.5 13.5h6v6h-6zM7 7h1M16 7h1M7 16h1M13.5 13.5h2.5v2.5M19.5 13.5v2.5M13.5 19.5h2.5M18.5 18.5h1v1",
```

`en.ts` `editor`, after `totpLabel`:

```ts
    scanQr: "Scan QR code (clipboard or screen)",
    scanning: "Looking for a QR code…",
    scanned: (label: string) => `${label} (scanned)`,
    scanPick: "Several QR codes found. Choose one:",
    scanUnnamed: "Unnamed",
    scanFailed: "Could not scan for a QR code.",
```

`pt-BR.ts` `editor`, after `totpLabel`:

```ts
    scanQr: "Ler QR code (área de transferência ou tela)",
    scanning: "Procurando um QR code…",
    scanned: (label: string) => `${label} (lido)`,
    scanPick: "Vários QR codes encontrados. Escolha um:",
    scanUnnamed: "Sem nome",
    scanFailed: "Não foi possível ler o QR code.",
```

- [ ] **Step 4: The editor UI**

In `ItemEditor.tsx`, next to the other `useState` calls:

```ts
  const [scanning, setScanning] = useState(false);
  const [scanChoices, setScanChoices] = useState<ScannedTotp[] | null>(null);
```

and, next to `generate()`:

```ts
  async function scanQr() {
    setScanning(true);
    setError(null);
    setScanChoices(null);
    try {
      const found = await api.scanTotpQr();
      if (found.length === 1) pickScan(found[0]!);
      else setScanChoices(found);
    } catch (e) {
      setError(errorMessage(e, t, t.editor.scanFailed));
    } finally {
      setScanning(false);
    }
  }

  function pickScan(code: ScannedTotp) {
    setScanChoices(null);
    setTotp({ mode: "scanned", token: code.token, label: scanLabel(code, t.editor.scanUnnamed) });
  }
```

(If `errorMessage` is not yet imported in `ItemEditor.tsx`, add `import { errorMessage } from "../i18n/errors";`. Use the same error display the editor already uses for save errors — `setError` feeding the existing error line.)

Replace the TOTP row's final `: (` branch (the bare `<input className="edit-input mono" type="password" … />`) with a `scanned` branch and an input-plus-button branch:

```tsx
              ) : totp.mode === "scanned" ? (
                <div className="edit-secret">
                  <span className="scanned-code">
                    <Icon name="check" size={15} /> {t.editor.scanned(totp.label)}
                  </span>
                  <span className="edit-secret-actions">
                    <button type="button" className="btn btn-small" onClick={() => setTotp(EMPTY)}>
                      {t.common.undo}
                    </button>
                  </span>
                </div>
              ) : (
                <div className="edit-secret">
                  <input
                    className="edit-input mono"
                    type="password"
                    value={totp.value}
                    onChange={(e) => setTotp({ mode: "set", value: e.target.value })}
                    placeholder={t.editor.totpPlaceholder}
                    autoComplete="off"
                    spellCheck={false}
                    autoCapitalize="off"
                    aria-label={t.editor.totpLabel}
                  />
                  {canScan(totp) && (
                    <span className="edit-secret-actions">
                      <button
                        type="button"
                        className="icon-btn"
                        onClick={() => void scanQr()}
                        disabled={scanning || readOnly}
                        aria-label={scanning ? t.editor.scanning : t.editor.scanQr}
                        title={scanning ? t.editor.scanning : t.editor.scanQr}
                      >
                        {scanning ? <span className="spinner" aria-hidden="true" /> : <Icon name="qr" size={16} />}
                      </button>
                    </span>
                  )}
                </div>
              )}
            </div>
            {scanChoices && scanChoices.length > 1 && (
              <div className="row scan-choices">
                <span className="muted">{t.editor.scanPick}</span>
                {scanChoices.map((code) => (
                  <button key={code.token} type="button" className="btn btn-small" onClick={() => pickScan(code)}>
                    {scanLabel(code, t.editor.scanUnnamed)}
                  </button>
                ))}
              </div>
            )}
```

(The closing `</div>` shown is the existing one of `<div className="row edit-row">`; the pick list is a sibling row inside the same `.group`.) Also clear `scanChoices` when the user types: in the input's `onChange`, add `setScanChoices(null);`.

`styles.css`, after the `.url-link:hover` rule:

```css
.scanned-code {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-strong);
  font-size: 14px;
}

.scanned-code svg {
  color: var(--brass);
}

.scan-choices {
  flex-wrap: wrap;
  gap: 8px;
}
```

- [ ] **Step 5: Verify**

Run: `cd apps/desktop && pnpm -s typecheck && pnpm -s test`
Expected: typecheck OK; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src
git commit -m "feat(desktop): QR button fills one-time codes from the clipboard or screen"
```

---

### Task 6: Security docs and the manual check

**Files:**
- Modify: `docs/security-model.md` (§6, §7)
- Modify: `docs/threat-model.md` (§3)

- [ ] **Step 1: Security model**

§6, after the opener bullet text added earlier, add a bullet:

```markdown
* **Screen capture (TOTP QR scan).** `scan_totp_qr` reads the clipboard
  image and, only if that holds no TOTP code, captures every monitor
  (`xcap`). It runs only on the user's click in the unlocked app; pixels
  stay in memory for that one call and are never written, logged or sent.
  Only text that parses as `otpauth://totp` survives decoding. macOS asks
  for Screen Recording once; Wayland's portal asks every time.
```

§7: change "all 38 of them" to "all 39 of them", and add the row after `open_website`:

```markdown
| `scan_totp_qr` | yes | no. Returns a token and issuer/account per code found; the `otpauth://` URI stays in Rust for 5 minutes, one scan at a time, cleared on lock. `create_item`/`update_item` accept `totp: { op: "scanned", value: token }` |
```

- [ ] **Step 2: Threat model**

In §3, under the adversary that covers a compromised renderer (search for "renderer"), add:

```markdown
* **QR scan as a screen reader.** A compromised renderer can call
  `scan_totp_qr` while the vault is unlocked, which captures the screen.
  It receives only tokens and issuer/account labels of TOTP QR codes, never
  pixels or other text, and a token only lets it save that code into an
  item, which it could already do by typing one.
```

- [ ] **Step 3: Full verification**

Run:

```bash
cd apps/desktop/src-tauri && cargo fmt --check && cargo clippy -q --all-targets && cargo test -q
cd .. && pnpm -s typecheck && pnpm -s test
cd ../.. && cargo deny check
grep -rn "log::\|println!\|eprintln!\|tracing::" apps/desktop/src-tauri/src/qr_scan.rs apps/desktop/src-tauri/src/scan_slot.rs apps/desktop/src-tauri/src/item_input.rs
```

Expected: all pass; the grep prints nothing.

- [ ] **Step 4: Manual check (the user, on WSLg)**

Ask the user to run `pnpm tauri dev` in `apps/desktop` and:
1. Open a test QR (e.g. generate `otpauth://totp/Test:me?secret=JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP&issuer=Test` in any QR generator) in a browser, with an empty clipboard or text on it. New login → QR button → "✓ Test · me (scanned)" → Save → the item shows a one-time code.
2. Screenshot the QR to the clipboard, hide the browser, open an existing login → Replace → QR button → found without the QR being on screen.
3. No QR anywhere → "No QR code found…".

- [ ] **Step 5: Commit and push**

```bash
git add docs/security-model.md docs/threat-model.md
git commit -m "docs: security notes for the TOTP QR scan"
git push origin main
```

# TOTP from a QR code — Design

Status: proposed, 2026-09-27.

> This software has not undergone an independent security audit.

## 1. Goal

Setting up two-factor on a website today means copying the `otpauth://` URI
or the Base32 secret by hand into the editor's one-time codes field. Most
sites show a QR code instead. HavenKeys should read that QR code directly,
with **one button** (a QR icon) in the one-time codes row, both when
creating a login and when editing one:

1. It first looks at the clipboard: if the user copied a screenshot of the
   QR code, that is used.
2. Otherwise it looks at every monitor and finds the QR code on screen,
   the way 1Password does.

Success: on a site's "set up authenticator" page, the user creates or
opens the login in HavenKeys, clicks the QR button, sees
"GitHub · you@example.com",
saves, and the item generates codes. The TOTP secret never enters the
WebView.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Buttons | One QR-icon button: clipboard image first, then screen | Separate "screen" and "clipboard" buttons |
| Screen mode | Capture all monitors, auto-find QR codes | Clipboard only; drag-a-region overlay |
| Where it appears | Editor, for new logins and when editing | Edit only |
| Where the secret goes | Stays in Rust in a one-time slot; the UI gets a token and a preview | Return the URI to the renderer to fill the field |
| Libraries | `xcap` (capture) + `rqrr` (decode) + existing `arboard` (clipboard image) | `rxing` (far larger than needed); per-OS native APIs (three implementations) |
| Several QR codes | List them as "Issuer · account", user picks; exactly one is selected immediately | Refuse; take the first |
| Core changes | None: the new variant exists only in the desktop's input type | `SecretUpdate::Scanned` in havenkeys-core |
| Linux capture dependency | Accept xcap's libpipewire-0.3 (build: libpipewire-0.3-dev, libclang-dev, libgbm-dev) | Own X11-only capture; clipboard only on Linux |

Out of scope: Google Authenticator export QRs (`otpauth-migration://`),
HOTP, scanning from a camera or an image file.

## 3. Flow

```text
Editor, new or existing login (TOTP field empty, or after "Replace")
  [QR icon button]
                          ▼
     scan_totp_qr                                             (Rust)
       1. vault must be unlocked
       2. clipboard holds an image? decode it (steps a–d)
            a. rqrr: find and decode every QR grid
            b. keep strings that parse via totp::parse_totp_input and
               start with otpauth://totp ; zeroize the decoded strings
            c. drop the pixel buffer
            d. dedupe by (secret, algorithm, digits, period)
       3. no TOTP found in the clipboard (no image, no QR, or no TOTP QR):
          capture every monitor and run a–d on each capture
       4. replace the slot: token (128-bit random) → TotpConfig, 5 min TTL
       ← [{ token, issuer, account }]
                          ▼
  one result → selected; several → pick list; none → error message
  field shows "✓ GitHub · you@example.com (scanned)" [Undo]
                          ▼
  create_item / update_item with totp: { op: "scanned", value: token }
       desktop maps it: take slot entry → SecretUpdate::Set(uri form)
       unknown / expired / used token → invalid_input, nothing saved
```

## 4. Components

### 4.1 `qr_scan.rs` (desktop, new)

* `fn decode_totp_qrs(images: &[RgbaImage]) -> Vec<TotpConfig>` — pure,
  unit-tested. Converts to luma, runs `rqrr::PreparedImage::detect_grids`,
  decodes each grid, keeps only valid TOTP configs, dedupes.
* `fn capture_screens() -> Result<Vec<RgbaImage>, ScanError>` — `xcap`
  `Monitor::all()` then `capture_image()` for each.
* `fn clipboard_image() -> Option<RgbaImage>` — `arboard` `get_image()`;
  no image, or an unreadable clipboard, is `None` and the scan moves on to
  the screen.
* `fn scan(clipboard, screens) -> Result<Vec<TotpConfig>, ScanError>` — the
  clipboard-then-screen order, with both sources injected so it is
  unit-tested without a display. The screen is captured only when the
  clipboard gave no TOTP code.
* Size guard: images above 64 megapixels in total are refused rather than
  decoded.

### 4.2 Scan slot (in `AppState`)

`Mutex<Option<ScanSlot>>` where `ScanSlot { created: Instant, entries:
Vec<(Token, TotpConfig)> }`.

* A scan replaces the whole slot.
* `take(token)` removes and returns that entry if the slot is less than 5
  minutes old; the whole slot is dropped after one successful take.
* Cleared on lock (same place the other decrypted state is dropped).
* Tokens are 16 bytes from the OS CSPRNG, hex in the IPC.
* `TotpConfig.secret` is a `SecretString`, zeroized on drop.

### 4.3 Command `scan_totp_qr`

* Declared in `build.rs`, granted in `capabilities/main.json`.
* Requires unlocked; calls `touch()`.
* Heavy work (capture, decode) runs off the main thread
  (`async` command + `spawn_blocking`).
* Takes no arguments.
* Returns `Vec<ScannedTotp { token, issuer: Option<String>, account:
  Option<String> }>`; an empty result is an error (§5), never `[]`.

### 4.4 Edit input

`create_item` / `update_item` take a desktop `ItemInputWire` whose `totp`
field is `keep | set | clear | scanned(token)`. Everything else is passed
through unchanged. `scanned` is resolved against the slot and becomes
`SecretUpdate::Set` with the scanned config rendered as an `otpauth://`
URI, so the core's existing parsing and validation run unchanged.

### 4.5 UI (`ItemEditor.tsx`)

* Whenever the field is in `set` mode and empty — a new login, a login
  without codes yet, or after **Replace** — the one-time codes row shows
  one icon button at the end of the field: a QR-code icon (new `qr` entry
  in `Icon.tsx`), tooltip and label "Scan QR code (clipboard or screen)".
* A scan in progress disables the button and shows a spinner in it.
* Several results: an inline list of "Issuer · account" (missing parts
  shown as "Unnamed"), click to pick.
* Picked: "✓ Issuer · account (scanned)" with **Undo**, which returns to
  the empty field. The renderer state holds only the token and the labels.
* Strings in `en` and `pt-BR`.

## 5. Errors

Fixed messages from Rust; never containing decoded text.

| Code | When | Message |
|---|---|---|
| `qr_not_found` | No QR code in the clipboard image (if any) or on screen | "No QR code found. Make sure it's fully visible on screen." On macOS also: "If this is the first scan, allow HavenKeys in System Settings → Privacy & Security → Screen Recording." |
| `qr_not_totp` | QR codes found (clipboard or screen), none a TOTP setup | "The QR code isn't a one-time code setup." |
| `screen_capture` | Clipboard gave nothing and the screen capture failed or was cancelled (Wayland portal, no display) | "Could not capture the screen." |

A clipboard without an image is not an error; the scan just continues to
the screen.
| `invalid_input` | Save with unknown, expired or used token | "The scanned code expired. Scan it again." |

macOS silently returns only the wallpaper when Screen Recording is denied,
so a denial is indistinguishable from "no QR code"; hence the hint.

## 6. Security

* **New capability: screen capture.** It runs only on an explicit click in
  the unlocked desktop app. The capture contains whatever else is on
  screen; it is held only in memory for the duration of the command,
  never written to disk, logged or sent anywhere, and only strings that
  parse as `otpauth://totp` survive decoding.
* **The secret stays in Rust.** The renderer receives a token and the
  issuer/account labels (already non-secret item metadata elsewhere). A
  compromised renderer can trigger a scan, but a scan returns no secret
  and a token only lets it save the scanned code into an item, which it
  could already do by typing one.
* **Slot lifetime:** one scan at a time, 5 minutes, one use, cleared on
  lock.
* **Decoded strings** are held in `Zeroizing<String>` until parsed.
* **Clipboard first** means a screenshot on the clipboard avoids capturing
  the screen at all. The clipboard is only read, never changed.
* **OS prompts:** macOS Screen Recording (once), Wayland portal (each
  scan that reaches the screen). No new Tauri plugin; no renderer permission besides
  `allow-scan-totp-qr`.
* **Dependencies:** `xcap`, `rqrr` must pass `cargo deny` (licenses,
  advisories, bans) before the feature lands. On Linux, `xcap` links
  `libpipewire-0.3` and `libgbm`; the .deb declares them.
* Docs: `security-model.md` §6 and §7 (command table, count),
  `threat-model.md` (screen capture as a deliberate, bounded capability).

## 7. Testing

**Rust unit (`qr_scan.rs`)**, images generated with the existing `qrcode`
crate:

* one TOTP QR → one config with issuer and account;
* two different TOTP QRs → both; the same one twice → one;
* URL QR, `otpauth://hotp`, `otpauth-migration://` → `qr_not_totp`;
* blank image → `qr_not_found`;
* order: TOTP QR on the clipboard → screen never captured; clipboard with
  no image / a non-TOTP QR → screen captured and its code returned;
  non-TOTP QR on the clipboard and nothing on screen → `qr_not_totp`;
* oversized input → refused.

**Rust unit (slot)**: token takes once; unknown token refused; expired
token refused (injected clock); lock empties the slot; a new scan
invalidates the old tokens.

**Rust unit (edit input)**: `scanned` resolves to `Set` with the same
secret; invalid token → error before anything is staged.

**Frontend**: `commands.test.ts` allowlist agreement; editor states
(QR button visible for new logins and when no code is kept, pick list, scanned
preview, undo).

**Manual**: on WSLg (X11), with a test QR open in a browser: an empty
clipboard → found on screen; a screenshot of it on the clipboard → found
without a screen capture. Both on a new login and on an existing one.

**Before merge**: `cargo deny check`, clippy, typecheck, and a grep that no
log line or error message formats decoded QR text.

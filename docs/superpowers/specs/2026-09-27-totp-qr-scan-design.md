# TOTP from a QR code — Design

Status: proposed, 2026-09-27.

> This software has not undergone an independent security audit.

## 1. Goal

Setting up two-factor on a website today means copying the `otpauth://` URI
or the Base32 secret by hand into the editor's one-time codes field. Most
sites show a QR code instead. HavenKeys should read that QR code directly:

* **Scan QR on screen** — one click; HavenKeys looks at every monitor and
  finds the QR code, the way 1Password does.
* **Paste QR image** — reads an image already on the clipboard (a
  screenshot the user took).

Success: on a site's "set up authenticator" page, the user opens the login
in HavenKeys, clicks **Scan QR on screen**, sees "GitHub · you@example.com",
saves, and the item generates codes. The TOTP secret never enters the
WebView.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Screen mode | Capture all monitors, auto-find QR codes | Clipboard only; drag-a-region overlay |
| Where the secret goes | Stays in Rust in a one-time slot; the UI gets a token and a preview | Return the URI to the renderer to fill the field |
| Libraries | `xcap` (capture) + `rqrr` (decode) + existing `arboard` (clipboard image) | `rxing` (far larger than needed); per-OS native APIs (three implementations) |
| Several QR codes | List them as "Issuer · account", user picks; exactly one is selected immediately | Refuse; take the first |
| Core changes | None: the new variant exists only in the desktop's input type | `SecretUpdate::Scanned` in havenkeys-core |

Out of scope: Google Authenticator export QRs (`otpauth-migration://`),
HOTP, scanning from a camera or an image file.

## 3. Flow

```text
Editor (TOTP field empty or "Replace")
  ├─ [Scan QR on screen] ─┐
  └─ [Paste QR image] ────┤
                          ▼
     scan_totp_qr { source: "screen" | "clipboard" }         (Rust)
       1. vault must be unlocked
       2. capture every monitor / read clipboard image → RGBA in memory
       3. rqrr: find and decode every QR grid
       4. keep strings that parse via totp::parse_totp_input and start
          with otpauth://totp ; zeroize the decoded strings
       5. drop pixel buffers
       6. dedupe by (secret, algorithm, digits, period)
       7. replace the slot: token (128-bit random) → TotpConfig, 5 min TTL
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
* `fn clipboard_image() -> Result<RgbaImage, ScanError>` — `arboard`
  `get_image()`.
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
* Returns `Vec<ScannedTotp { token, issuer: Option<String>, account:
  Option<String> }>`; an empty result is an error (§5), never `[]`.

### 4.4 Edit input

`create_item` / `update_item` take a desktop `ItemInputWire` whose `totp`
field is `keep | set | clear | scanned(token)`. Everything else is passed
through unchanged. `scanned` is resolved against the slot and becomes
`SecretUpdate::Set` with the scanned config rendered as an `otpauth://`
URI, so the core's existing parsing and validation run unchanged.

### 4.5 UI (`ItemEditor.tsx`)

* Whenever the field is in `set` mode and empty (a new login, or after
  **Replace**), the one-time codes row shows two small buttons: **Scan QR on screen**,
  **Paste QR image**.
* A scan in progress disables both and shows "Scanning…".
* Several results: an inline list of "Issuer · account" (missing parts
  shown as "Unnamed"), click to pick.
* Picked: "✓ Issuer · account (scanned)" with **Undo**, which returns to
  the empty field. The renderer state holds only the token and the labels.
* Strings in `en` and `pt-BR`.

## 5. Errors

Fixed messages from Rust; never containing decoded text.

| Code | When | Message |
|---|---|---|
| `qr_not_found` | No QR code in any image | "No QR code found. Make sure it's fully visible on screen." On macOS also: "If this is the first scan, allow HavenKeys in System Settings → Privacy & Security → Screen Recording." |
| `qr_not_totp` | QR codes found, none a TOTP setup | "The QR code isn't a one-time code setup." |
| `clipboard_no_image` | Clipboard has no image | "The clipboard doesn't hold an image." |
| `screen_capture` | Capture failed or was cancelled (Wayland portal, no display) | "Could not capture the screen." |
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
* **OS prompts:** macOS Screen Recording (once), Wayland portal (each
  scan). No new Tauri plugin; no renderer permission besides
  `allow-scan-totp-qr`.
* **Dependencies:** `xcap`, `rqrr` must pass `cargo deny` (licenses,
  advisories, bans) before the feature lands.
* Docs: `security-model.md` §6 and §7 (command table, count),
  `threat-model.md` (screen capture as a deliberate, bounded capability).

## 7. Testing

**Rust unit (`qr_scan.rs`)**, images generated with the existing `qrcode`
crate:

* one TOTP QR → one config with issuer and account;
* two different TOTP QRs → both; the same one twice → one;
* URL QR, `otpauth://hotp`, `otpauth-migration://` → `qr_not_totp`;
* blank image → `qr_not_found`;
* oversized input → refused.

**Rust unit (slot)**: token takes once; unknown token refused; expired
token refused (injected clock); lock empties the slot; a new scan
invalidates the old tokens.

**Rust unit (edit input)**: `scanned` resolves to `Set` with the same
secret; invalid token → error before anything is staged.

**Frontend**: `commands.test.ts` allowlist agreement; editor states
(scan buttons visible only when no code is kept, pick list, scanned
preview, undo).

**Manual**: screen scan on WSLg (X11) with a test QR open in a browser;
clipboard scan with a screenshot.

**Before merge**: `cargo deny check`, clippy, typecheck, and a grep that no
log line or error message formats decoded QR text.

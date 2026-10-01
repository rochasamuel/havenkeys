# Android app — Design

Status: proposed, 2026-10-01.
Amends `CLAUDE.md`, which lists mobile as out of scope for the MVP (see §13).

> This software has not undergone an independent security audit.

## 1. Goal

HavenKeys gets an Android app (`net.havenkeys.android`) that is a full client
of the same `havenkeys-server` account as the desktop app. A user can:

1. Sign in on the phone with the Emergency Kit and the master password.
2. Unlock with the master password or, once enabled, a fingerprint or face.
3. Browse, search, reveal, copy and read TOTP codes, offline.
4. Fill logins and TOTP codes in apps and browsers through Android Autofill.
5. Create, edit and delete items, and save logins Autofill detects.
6. Create and use passkeys through Android Credential Manager (Android 14+).
7. Fill cards and identities.

iOS comes later, as a Swift shell over the same Rust API (§4.2). Nothing in
this design is Android-only below the Kotlin layer.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Repository | This monorepo | A separate repo (the Rust core and sync client would drift) |
| First platform | Android | iOS first; both at once |
| Where security logic lives | Rust, shared with desktop | Kotlin orchestration over low-level core calls |
| Account/sync orchestration | Extracted from `src-tauri` into `havenkeys-client`, used by desktop and mobile | Copying it into the mobile crate |
| UI toolkit | Fully native: Jetpack Compose on Android, SwiftUI on iOS | Flutter (the autofill and passkey screens must be native anyway, so it would mean two UI toolkits per platform and one more layer — Dart — holding secrets) |
| Native binding | UniFFI (Kotlin now, Swift later), called directly | flutter_rust_bridge; a channel layer between UI and native |
| Unlock | Master password; biometric unlock via Keystore after opt-in; password again every 14 days and after reboot | Password only; biometric with no re-entry |
| Unlocked autofill | Matched items' values handed to the Android autofill framework; "Confirm before filling" restores per-fill authentication | Always authenticate each dataset |
| App ↔ site association | Encrypted app bindings (package + signing cert) plus Digital Asset Links verification | Package name alone; trusting `webDomain` from any app |
| Auto sign-in (pressing buttons) | Not on Android | An Accessibility Service |
| Distribution | Signed APK on GitHub Releases, Play-ready (`github` and `play` flavors) | Play Store first |
| Minimum Android | 9 (API 28); passkeys on 14+ (API 34) | — |

## 3. Milestones

| | Scope |
|---|---|
| Step 0 | Extract `havenkeys-client`; desktop behaviour unchanged |
| M1 | Onboarding, unlock + biometrics, vault viewer, Autofill for logins and TOTP, Digital Asset Links, direct fill |
| M2 | Create/edit/delete, save from Autofill |
| M3 | Passkeys and passwords through Credential Manager |
| M4 | Card and identity autofill |

Each milestone gets its own implementation plan and is usable when it ships.

## 4. Architecture

### 4.1 `havenkeys-client` (step 0)

The account logic in `apps/desktop/src-tauri` is not Tauri-specific and moves
into `crates/havenkeys-client`:

* activation, second-device sign-in, `connect`, `sync_now`, `push`,
  `push_batches`, `ensure_identity` (`account.rs`, `sync.rs`);
* device list, revoke, sign out, remove device (`account.rs`, `removal.rs`);
* `Connectivity` and the session token (`state.rs`);
* the device ID and Secret Key record (`device.rs`).

Platform differences become traits the shell implements:

```rust
trait SecretKeyStore   // desktop: OS keychain + file fallback; Android: Keystore-encrypted file
trait ClientEvents     // desktop: app.emit(...); Android: a UniFFI callback
```

The desktop keeps its Tauri commands as thin wrappers. Step 0 is done when
the existing desktop and bridge tests pass unchanged and the desktop behaves
identically; no mobile code exists yet.

### 4.2 `havenkeys-mobile`

A UniFFI crate: the only Rust surface Kotlin (and later Swift) can call. Its
API is intent-level and mirrors the bridge's split between listing and
filling:

```text
status, onboard_from_kit, activate, sign_in, unlock_password,
unlock_bundle_create, unlock_with_bundle, lock,
list_items, search, item_view, reveal(id, field), totp(id),
generate_password,
autofill_matches(target), autofill_fill(id, target), autofill_totp(id, target),
bind_app(id, app), stage/save/edit/delete (M2),
passkey_* (M3), card/identity fill (M4),
sync_now, devices, sign_out, remove_device, settings
```

It never returns keys, blobs, or whole vault objects. Every call that returns
a secret validates the lock state, the item and the target in Rust.

There is one `VaultService` per app process. The app's screens, the
`AutofillService` and the `CredentialProviderService` all run in the app's
default process and share it: one unlock opens all three, one lock closes
all three.

### 4.3 Layout

```text
crates/havenkeys-client/     account, sync, session (shared with desktop)
crates/havenkeys-mobile/     UniFFI API
apps/android/                Gradle project, application id net.havenkeys.android
  app/src/main/kotlin/net/havenkeys/android/
    data/                    repositories over the UniFFI API (the only door to Rust)
    ui/<feature>/            <Feature>Screen.kt + <Feature>ViewModel.kt (§9)
    ui/theme/                design tokens, typography, motion
    autofill/                AutofillService, structure parser, field classifier
    credentials/             CredentialProviderService (M3)
    security/                Keystore, BiometricGate, SecretKeyStore impl
    update/                  GitHub update check (github flavor only)
    AppContainer.kt          manual dependency wiring
apps/ios/                    later: SwiftUI app + AutoFill extension over the same UniFFI API
```

### 4.4 Boundaries

* **Rust** decides: matching, origin and app checks, what a fill returns,
  bundle validity, passkey RP authorization.
* **Kotlin services** collect facts from Android (screen structure, caller
  package and certificates) and talk to Keystore and BiometricPrompt. They
  hold a secret only while passing it from Rust to Android for a fill.
* **Kotlin UI** (Compose) draws screens. ViewModels hold item overviews; a
  secret reaches the UI only when the user taps reveal or copy (§9.4).

## 5. Unlock

### 5.1 Master password

As on desktop: `MK = Argon2id(password)`, KEK from MK and the Secret Key
(key scheme 3, `docs/crypto.md`), the vault key unwrapped from the header.
The auth key from the same derivation signs in to the server. Required after
sign-in, after a reboot, every 14 days, and whenever the unlock bundle is
refused.

### 5.2 Biometric unlock

Opt-in from Settings, with the master password entered once.

**Keystore key:** AES-256-GCM, StrongBox when available,
`setUserAuthenticationRequired(true)` with `BIOMETRIC_STRONG` and no validity
window (every use needs a `CryptoObject`-bound prompt),
`setInvalidatedByBiometricEnrollment(true)`, `setUnlockedDeviceRequired(true)`.

**Unlock bundle**, produced by Rust and encrypted by the Keystore key into
app-private storage:

```text
version, vault key, auth key, enrolled_at (Unix ms), boot_count
```

**Unlocking:** BiometricPrompt → Keystore decrypts → bytes passed straight to
`unlock_with_bundle` → Kotlin zeroes its `ByteArray`. Rust refuses a bundle
older than 14 days or from another boot (`Settings.Global.BOOT_COUNT`);
Kotlin then deletes it and asks for the master password. A Keystore key
invalidated by a new enrollment is handled the same way.

The auth key is in the bundle so a biometric unlock can sign in and write. If
the master password changed on another device the server refuses it; the
bundle is deleted and the password is asked for once.

**Stated cost:** someone who passes this phone's biometric check gets what
the master password gives — the vault and a server session — on this phone
only. The master password is never stored.

**Limitation:** the decrypted bundle passes briefly through JVM memory. It is
zeroed at once, but the JVM gives no guarantee about copies; this joins the
zeroization limits already in `security-model.md`.

### 5.3 Locking

The core `LockManager` and its settings (never, 5, 15, 30, 60 minutes), plus:

* lock when the screen turns off (setting, on by default);
* the process dying locks (the vault exists only in memory);
* the 14-day expiry deletes the bundle.

Locking zeroes the Rust session and drops the server token. A locked Autofill
or Credential Manager request shows "Unlock HavenKeys".

## 6. Sync, editing and network

### 6.1 Sync

`havenkeys-client`, identical to desktop. The replica is the schema-4 SQLite
in app-private storage. Pull runs on unlock, on returning to the foreground,
on pull-to-refresh, and every few minutes while unlocked and in the
foreground. Nothing syncs while locked.

### 6.2 Editing (M2)

Logins, notes, cards and identities through `stage_* → push → commit`. A
conflict shows "Changed on another device" and reloads. Offline disables
editing with a banner; reading, reveal, TOTP and autofill always work offline.

### 6.3 Network

* rustls with `rustls-platform-verifier`, so Android's trust store decides.
* `network_security_config` forbids cleartext; debug builds allow `http://`
  for local servers only.
* Outbound connections, exhaustively: the user's server; `assetlinks.json`
  from sites in the vault (setting, on by default); GitHub Releases
  (`github` flavor, setting, on by default). No analytics, no crash reporting.

### 6.4 Onboarding and removal

* Scan the Emergency Kit QR code (`havenkeys://kit/v2?...`: server, account,
  email, Secret Key), then type the master password; or type all four.
* Activate from an invite, as on desktop.
* Sign out and Remove device follow the desktop flow and also delete the
  Keystore key, the unlock bundle and the Secret Key file.

### 6.5 Android hardening

* `FLAG_SECURE` on every activity: no screenshots, blank recents thumbnail.
* `allowBackup="false"` and data-extraction rules excluding everything.
* Exported components: only the autofill and credential services, each
  guarded by its system permission (`BIND_AUTOFILL_SERVICE`,
  `BIND_CREDENTIAL_PROVIDER_SERVICE`), and the launcher activity.
* Permissions: `INTERNET`, `USE_BIOMETRIC`, `CAMERA` (QR scan, requested when
  scanning), `REQUEST_INSTALL_PACKAGES` (`github` flavor only).

## 7. Autofill (M1, save in M2)

### 7.1 Parsing

A Kotlin `AutofillService` walks the `AssistStructure` and builds a fact list
per field: autofill hints, input type, resource id, hint text, HTML
attributes, visibility, focus, order. A `FieldClassifier` scores the fields
into username/email, password, new password, confirmation, OTP — several
signals, never one — like the extension's engine (`docs/autofill.md`). The
facts are untrusted input.

### 7.2 Target

The fill target is decided in Rust from the caller's package name and
signing certificate SHA-256 and the structure's `webDomain`:

* **Browser target:** the package and certificate are on a fixed list of
  browsers in Rust (Chrome, Firefox, Edge, Brave, Samsung Internet, …). The
  `webDomain` is then matched with `origin.rs` exactly as the extension's page
  URL is.
* **App target:** everything else, including WebViews inside apps — the app
  controls its WebView, so its `webDomain` is not trusted. An app target
  matches:
  * items with an **app binding** for that package *and* certificate, stored
    inside the item's encrypted details; or
  * items whose website verifies the app through **Digital Asset Links**:
    `https://<domain>/.well-known/assetlinks.json` lists the package and
    certificate with `delegate_permission/common.handle_all_urls` or
    `common.get_login_creds`. Fetched over HTTPS only, size-limited, cached
    (encrypted, in the replica's settings blob) for 7 days, parsed in Rust.
* **Manual binding:** the dropdown always offers "Search HavenKeys…". Picking
  an item asks "Use GitHub in com.github.android?"; confirming stores the
  binding (an item write, so it needs to be online — offline, the fill
  happens once and nothing is stored).

### 7.3 Fill

* **Unlocked, default ("direct fill"):** the response carries one dataset per
  matched item with its username and password. The requesting app receives
  only the dataset the user taps. The Android autofill framework — part of
  the OS, already trusted with every keystroke — holds the values of the
  matched items (usually 1–3) for that fill session. This relaxes CLAUDE.md
  §33 for the OS only; see §13.
* **"Confirm before filling" on, or locked:** each dataset is
  authentication-gated. Tapping opens a minimal HavenKeys activity (biometric
  if locked), which calls `autofill_fill(id, target)`; Rust re-checks the
  target and returns username and password only.
* Inline suggestions (keyboard chips) on Android 11+, dropdown otherwise.
* Nothing is filled without a tap.

### 7.4 TOTP

When an OTP field appears for the same target after a fill, the response
offers the item's code from `autofill_totp`. "Copy code" marks the clip
sensitive (`ClipDescription.EXTRA_IS_SENSITIVE`, Android 13+) and clears it
after the configured time.

### 7.5 Save (M2)

Login forms get a `SaveInfo`. Android's own save sheet is the user's
confirmation; `onSaveRequest` passes the values and the target to Rust
`stage_save_login`, which binds the new item to the domain (browser) or
app binding (app) and pushes it. Offline answers "Can't save while offline".

### 7.6 Cards and identities (M4)

Standard Android hints (card number, expiry, security code, name, address,
phone, email) plus the classification already in `card_page.rs` and
`identity_page.rs`. The extension's rules carry over: cards only in https
browser targets or app targets, nothing overwritten, document numbers need a
second confirmation, nothing submitted. A card typed into a form is saved
only after confirmation.

### 7.7 Not on Android

Auto sign-in, "Sign in with" provider flows: Android autofill cannot press
buttons, and the only way to do it — an Accessibility Service — would give
HavenKeys control of every screen. Not done.

## 8. Passkeys (M3, Android 14+)

A `CredentialProviderService` registered for passkeys and passwords. Keys,
signing and storage are the existing Rust passkey code (spec
`2026-09-23-passkeys-design.md`): ES256, attestation `none`, counter 0, keys
inside the login's encrypted details. Passkeys are shared with desktop and
the extension through the server.

### 8.1 Caller

* **Browser:** the caller is on Rust's privileged browser list, and its
  reported origin is passed to `authorize_rp(rp_id, page_url)` — the same
  rule as the extension.
* **App:** origin `android:apk-key-hash:<base64url SHA-256 of the cert>`. The
  RP ID is allowed only if `https://<rp_id>/.well-known/assetlinks.json`
  lists the package and certificate with `common.get_login_creds`, through
  the §7.2 module. Otherwise the request is refused.

### 8.2 Flows

* **Create:** "Create passkey in HavenKeys?" → choose the login (or a new
  one) → biometric → Rust generates and stages → push. Online only.
* **Sign in:** the sheet lists HavenKeys passkeys (user name and site only) →
  tap → biometric → Rust signs. Works offline.
* User verification is a real `BiometricPrompt` (device credential fallback
  allowed) on every passkey use, even when unlocked.
* Password requests from apps using Credential Manager get logins through the
  §7.2 target rules.

Hybrid (QR from a computer) is Android's job and out of scope.

## 9. Android app UI

### 9.1 Screens

Onboarding (scan kit, type kit, invite) · Unlock · Vault (search; Logins,
Notes, Cards, Identities, Passkeys; offline/sync banner) · Item detail
(hidden fields, reveal, copy with clear, live TOTP) · Edit/create (M2, with
generator) · Generator · Settings (auto-lock, lock on screen off, biometrics,
Confirm before filling, Digital Asset Links, updates, account and devices,
sign out, remove device) · Autofill setup (links to Android's Autofill
service and Passwords & passkeys settings; Chrome's "Autofill using another
service"). The autofill picker, "Search HavenKeys…" and the passkey sheet
reuse the same Compose components.

### 9.2 Design

Jetpack Compose with Material 3. The visual design is done with the
`/impeccable` skill at implementation time, starting from the existing
identity (desktop app, `packages/ui` tokens, `apps/web/DESIGN.md`) and
adapted to Android. Smooth, purposeful motion for screen and state changes
(unlock → vault, list → detail with shared elements, reveal, TOTP progress,
the lock wipe), honouring Android's "Remove animations". Light and dark
follow the system. English and pt-BR as Android string resources
(`values/`, `values-pt-rBR/`), reusing the desktop's wording.

### 9.3 Architecture

Android's recommended app architecture
(`developer.android.com/topic/architecture`):

* **UI layer:** a `@Composable` screen per feature and a `ViewModel` that
  exposes one immutable `UiState` as a `StateFlow`. Composables hold layout,
  animation and simple conditions only; logic lives in the ViewModel.
* **Data layer:** repositories (`VaultRepository`, `AccountRepository`,
  `SettingsRepository`) as interfaces with one implementation over the
  UniFFI API, and fakes for tests. No local storage of their own: storage
  and offline behaviour belong to Rust.
* Unidirectional data flow: state flows down from repositories to
  ViewModels to screens; user actions flow up as ViewModel function calls.
* Coroutines throughout; Rust calls run on `Dispatchers.IO`; results are a
  `sealed` type, not exceptions, between layers.
* Navigation Compose; manual dependency wiring in `AppContainer` (no Hilt).
* Not used: Room, DataStore or SharedPreferences for anything from the vault.

### 9.4 Secret handling in the UI

* Repositories and ViewModel state expose overviews only.
* A revealed value lives in the `remember`ed state of the composable showing
  it and is cleared when it leaves the screen, after 30 seconds, or on lock.
* Never in ViewModel state, navigation arguments, `SavedStateHandle`, logs
  or exceptions.
* The lock event from Rust is a `StateFlow` every ViewModel observes: it
  clears their state and navigation returns to Unlock.
* Release builds are minified with R8.

## 10. Kotlin

* Coroutines and `suspend` instead of callbacks; `sealed` types for states and
  results; small single-purpose classes (`AutofillParser`, `FieldClassifier`,
  `KeystoreVault`, `BiometricGate`, `UpdateChecker`).
* Manual construction, no DI framework.
* No logging of vault data: detekt's `ForbiddenMethodCall` forbids
  `android.util.Log` and `println` in the whole app.

## 11. Code style

Simple, readable code with clear names, in Kotlin, Swift and Rust. Comments
only where the reason is not obvious — mostly security reasons. No
narration.

## 12. Build, release, testing

### 12.1 Build

`scripts/build-android.sh`: `cargo ndk` for arm64-v8a, armeabi-v7a, x86_64;
UniFFI Kotlin generation; `./gradlew assembleGithubRelease`. Generated
bindings are committed. CI builds and tests on Linux on every push.

### 12.2 Release

* Signed APK on the same GitHub Releases as desktop, with SHA-256 checksums.
* The release key stays outside the repo with an offline backup; custody is
  documented in `docs/development.md`. The same key is uploaded to Play App
  Signing if the app moves to Play, so sideloaded installs keep updating.
* `github` flavor: checks Releases, verifies the downloaded APK's signing
  certificate matches the installed app's, installs only after a tap; can be
  turned off. `play` flavor: no updater.

### 12.3 Tests

* **Rust:** existing suites pass after step 0; new tests for the mobile API,
  the unlock bundle (expiry, boot count, tampering, wrong key), browser list
  and app-binding matching, Digital Asset Links parsing (fuzzed), and Android
  passkey origins.
* **Security regressions (CLAUDE.md §46, Android):** an app with GitHub's
  package name and another certificate gets nothing; a WebView claiming
  `github.com` gets nothing; a locked vault fills nothing; a stale or tampered
  bundle is refused; an app not vouched for by `assetlinks.json` gets no
  passkey.
* **Kotlin:** unit tests for parsing and classification on recorded
  structures (login, email-first, password-only, OTP, WebView, Chrome);
  instrumented emulator tests for Keystore, biometrics and the services.
* **UI:** ViewModel unit tests with fake repositories
  (`kotlinx-coroutines-test`); Compose UI tests for navigation and the lock
  wipe.
* **Manual checklist** in `security-review.md`: a real phone; Chrome and
  Firefox; the GitHub and Google apps; passkeys on webauthn.io and github.com.

## 13. Documentation and CLAUDE.md amendment

* `CLAUDE.md`: amendment note — the Android app is in scope; iOS later; and
  the §33 relaxation for direct fill (values of matched items handed to the
  OS autofill framework while unlocked).
* `threat-model.md`, `security-model.md`: the unlock bundle, direct fill,
  app bindings, Digital Asset Links requests, permissions and exported
  services.
* `architecture.md`: `havenkeys-client`, `havenkeys-mobile`, the app.
* New `docs/android.md`; `README.md`.

## 14. Out of scope

iOS (next, over the same API); Play Store publishing; Wear OS; auto sign-in;
an Accessibility Service; background sync while locked; tablets beyond what
the phone layout gives; passkey import/export.

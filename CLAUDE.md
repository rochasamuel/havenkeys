# Secure Personal Password Manager — MVP

You are a senior security engineer and full-stack developer. Build a production-quality MVP of a **local-first personal password manager**, inspired by the UX and autofill quality of 1Password.

This application will store extremely sensitive information:

* Passwords
* Usernames
* TOTP secrets
* Secure notes
* Encryption keys

Therefore, **security is the highest priority**.

Do not optimize for development speed at the expense of security.

Do not invent cryptography.

Do not implement security mechanisms from scratch when a well-established audited library exists.

The application should initially support:

1. Desktop application
2. Browser extension
3. Secure communication between desktop and extension
4. Login/password management
5. Secure notes
6. Password generation
7. TOTP
8. High-quality autofill

Mobile, cloud synchronization, accounts, sharing, passkeys and other advanced functionality are explicitly OUT OF SCOPE for this MVP.

---

# 1. Core philosophy

The application should be:

* Local-first
* Zero-knowledge by architecture
* Offline-capable
* Encrypted at rest
* Minimal network exposure
* Minimal extension permissions
* Explicit-user-action based
* Resistant to malicious webpages
* Resistant to accidental secret leakage
* Easy to audit
* Modular enough to add synchronization/mobile later

Reads work without an internet connection: each device keeps an encrypted
read-only replica, so unlocking, searching, revealing, TOTP and autofill all
work offline. Changes require the server, which is the single writer.

The backend is part of the product: a small server the user runs themselves
(`havenkeys-server`), which stores ciphertext it cannot open. It is not a
hosted service and there is no vendor account.

> Amended on 2026-09-20 by
> `docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md`
> §10. The original text ("works completely without an internet connection",
> "no backend required") described the folder-sync design that spec replaced.

Do not introduce:

* Cloud accounts
* User registration
* Remote authentication
* Analytics
* Telemetry
* Crash reporting that may capture secrets
* Advertising
* Third-party APIs
* Remote databases

unless explicitly required later.

---

# 2. Recommended architecture

Use:

## Desktop

* Tauri
* React
* TypeScript
* Rust
* SQLite

## Browser extension

* TypeScript
* WebExtension APIs
* Manifest V3 for Chromium
* Firefox-compatible architecture

## Security-sensitive functionality

Prefer Rust.

The Rust layer should own:

* Key derivation
* Encryption/decryption
* Vault access
* Secure serialization
* Password generation
* TOTP generation where practical
* Sensitive vault operations
* Native messaging interface
* Lock state

The React frontend should NOT implement cryptography itself.

The browser extension should NOT directly access the encrypted database.

Architecture:

```text
┌───────────────────────────────────────┐
│             Desktop App               │
│                                       │
│  React / TypeScript UI                │
│              │                        │
│              ▼                        │
│        Tauri Commands                 │
│              │                        │
│              ▼                        │
│       Rust Security Core              │
│              │                        │
│       ┌──────┴────────┐               │
│       │               │               │
│   Crypto           Vault              │
│       │               │               │
│       └──────┬────────┘               │
│              │                        │
│              ▼                        │
│       Encrypted SQLite               │
└──────────────┬────────────────────────┘
               │
        Native Messaging
               │
               ▼
┌───────────────────────────────────────┐
│          Browser Extension            │
│                                       │
│ Background Service Worker             │
│ Content Scripts                       │
│ Popup                                 │
│ Autofill Engine                       │
└───────────────────────────────────────┘
```

---

# 3. Repository structure

Use a monorepo.

Suggested structure:

```text
/
├── apps/
│   ├── desktop/
│   │   ├── src/
│   │   └── src-tauri/
│   │
│   └── extension/
│       ├── src/
│       │   ├── background/
│       │   ├── content/
│       │   ├── popup/
│       │   ├── autofill/
│       │   └── messaging/
│       └── manifest/
│
├── packages/
│   ├── protocol/
│   ├── domain/
│   └── test-utils/
│
├── docs/
│   ├── architecture.md
│   ├── security-model.md
│   ├── threat-model.md
│   └── autofill.md
│
├── Cargo.toml
├── package.json
└── README.md
```

Keep security-sensitive code isolated and easy to audit.

---

# 4. Threat model

Before implementing the application, create:

```text
docs/threat-model.md
docs/security-model.md
```

Explicitly document the following threats.

## Protect against

### Local attackers

Assume an attacker may obtain a copy of the encrypted vault database.

They must NOT be able to recover plaintext secrets without the user's credentials/key material.

### Malicious webpages

A webpage must never be trusted.

Assume webpages may contain malicious JavaScript attempting to:

* Read credentials
* Trigger autofill
* Intercept credentials
* Communicate with the extension
* Manipulate DOM elements
* Create fake login forms
* Use hidden inputs
* Use iframes
* Attempt phishing
* Exploit content-script behavior

### Malicious browser content

The extension must assume that every webpage is potentially hostile.

### Extension compromise

Minimize the amount of sensitive data exposed to the extension.

### Process compromise

Do not assume the renderer/UI process is fully trusted.

Keep security-sensitive operations in Rust where practical.

### Accidental disclosure

Prevent secrets from appearing in:

* Logs
* Console output
* Error messages
* Crash reports
* URLs
* Clipboard longer than necessary
* Debugging output
* React state snapshots
* Browser extension storage
* SQLite plaintext columns

---

# 5. Cryptography

DO NOT invent cryptography.

Use established, well-maintained libraries.

Preferred primitives:

## Password-based key derivation

Use:

```text
Argon2id
```

with a documented configuration appropriate for the target platform.

Do not use:

* MD5
* SHA-1
* plain SHA-256
* PBKDF2 unless there is a compelling compatibility reason
* custom KDFs

The exact Argon2id parameters should be configurable/documented and tested on target hardware.

Never hardcode assumptions about "fast enough" without benchmarking.

---

# 6. Encryption

Use an authenticated encryption construction such as:

```text
AES-256-GCM
```

or another well-established AEAD construction supported by the selected audited Rust cryptography library.

Every encrypted object must include a unique nonce/IV.

NEVER reuse a nonce with the same encryption key.

The encrypted representation should contain enough metadata to support future migrations.

Conceptually:

```text
EncryptedBlob:

version
algorithm
nonce
ciphertext
authentication_tag
```

Do not store encryption keys alongside encrypted data.

---

# 7. Key hierarchy

Do NOT simply encrypt the entire vault using:

```text
AES(master_password)
```

Design a proper key hierarchy.

Recommended conceptual model:

```text
                    Master Password
                           │
                           ▼
                       Argon2id
                           │
                           ▼
                    Master Key
                           │
                           ▼
                    Key Encryption Key
                           │
                           ▼
                       Vault Key
                           │
                           ▼
                  Encrypted Vault Data
```

The vault key should be randomly generated using a cryptographically secure random number generator.

The master password should never itself be used directly as an encryption key.

Document the key hierarchy thoroughly.

---

# 8. Optional device secret

Design the architecture so a future version can introduce a device-specific secret / Secret Key without breaking the vault format.

Do NOT make the current implementation unnecessarily complicated, but keep the cryptographic format versioned.

Potential future model:

```text
Master Password
       +
Device Secret
       ↓
Key Derivation
       ↓
Vault Key
```

This should be documented as a future extension rather than implemented unnecessarily in MVP.

---

# 9. Master password handling

The master password is extremely sensitive.

Rules:

* Never log it.
* Never persist it in plaintext.
* Never store it in localStorage.
* Never store it in IndexedDB.
* Never send it to the browser extension.
* Never expose it to webpage JavaScript.
* Never include it in error messages.
* Never include it in telemetry.
* Clear temporary buffers when practical.

Use secure memory handling where practical in Rust.

Understand the limitations of memory zeroization in high-level environments and document them.

Do not claim that memory can be perfectly wiped on all platforms.

---

# 10. Vault format

Use a versioned vault format.

Example conceptual structure:

```text
Vault
├── format_version
├── vault_id
├── encrypted_vault_key
├── encrypted_items
└── metadata
```

Every vault item should have a stable UUID.

Example:

```ts
type VaultItem = {
  id: string;
  type: "login" | "secure_note";
  createdAt: number;
  updatedAt: number;
};
```

Sensitive fields must reside inside the encrypted payload.

---

# 11. Login item

Support:

```ts
type LoginItem = {
  id: string;
  type: "login";

  title: string;

  urls: Array<{
    url: string;
    matchType: "exact" | "origin" | "domain";
  }>;

  username?: string;
  password?: string;

  totp?: {
    secret: string;
    algorithm: "SHA1" | "SHA256" | "SHA512";
    digits: 6 | 8;
    period: number;
  };

  notes?: string;

  createdAt: number;
  updatedAt: number;
};
```

The actual persisted encrypted representation can differ.

The plaintext domain/title metadata should be minimized.

---

# 12. Secure notes

Support secure notes containing arbitrary text.

Example:

```text
Title
Content
Created
Updated
```

The entire content must be encrypted.

Do not index secure-note content in an unencrypted search index.

For MVP, decrypt notes only when necessary.

---

# 13. Password generator

Implement a cryptographically secure password generator.

Use the operating system / Rust cryptographically secure random number generator.

Support:

* Length
* Uppercase
* Lowercase
* Numbers
* Symbols

Example:

```text
Length: 24
Uppercase: ✓
Lowercase: ✓
Numbers: ✓
Symbols: ✓
```

Avoid modulo bias when randomly selecting characters.

Do not use:

```text
Math.random()
```

or equivalent insecure randomness.

---

# 14. TOTP

Support standard TOTP.

Support:

* SHA-1
* SHA-256
* SHA-512
* 6 digits
* 8 digits
* Configurable period

Support parsing:

```text
otpauth://
```

Generate codes according to the standard.

Never log TOTP secrets or generated codes.

The extension should be able to request a TOTP code for a specific authenticated vault item.

---

# 15. Desktop lock state

The vault must have explicit states:

```text
LOCKED
UNLOCKING
UNLOCKED
LOCKING
```

When locked:

* Destroy decrypted vault state where practical.
* Clear sensitive caches.
* Clear decrypted item collections.
* Invalidate active extension authorization.
* Do not allow secret retrieval.

The UI should clearly indicate locked/unlocked state.

---

# 16. Auto-lock

Support configurable auto-lock:

```text
Never
5 minutes
15 minutes
30 minutes
1 hour
```

Also lock when:

* User explicitly locks
* Application exits
* Relevant OS/session lock events occur where supported

Design the lock manager as an independent module.

---

# 17. Browser extension architecture

Use Manifest V3.

Separate:

```text
Background Service Worker
Content Script
Popup UI
Autofill Engine
Messaging Layer
```

Never expose the vault directly to webpage JavaScript.

Content scripts should have the minimum possible permissions.

Do NOT inject broad scripts unnecessarily.

---

# 18. Extension permissions

Request the minimum browser permissions necessary.

Do NOT blindly request:

```text
<all_urls>
```

unless absolutely necessary.

Investigate the minimum permission model that still permits high-quality autofill.

Clearly document every permission in:

```text
docs/security-model.md
```

---

# 19. Content script security

Treat webpage DOM as untrusted input.

Never trust:

* Input values
* Element names
* Element IDs
* Form actions
* Page URLs
* iframe URLs
* DOM attributes

Do not insert vault data using unsafe HTML APIs.

Avoid:

```js
innerHTML
```

for sensitive UI.

Never expose secrets through DOM attributes.

Never put passwords in:

```text
data-*
```

attributes.

Never put passwords in URLs.

---

# 20. Autofill engine

This is one of the most important parts of the project.

The goal is to achieve an experience conceptually similar to 1Password.

Do not implement a naive:

```js
document.querySelector('input[type=password]')
```

solution.

Build a dedicated autofill engine.

It should identify:

* Username fields
* Email fields
* Password fields
* Current password fields
* New password fields
* Confirmation password fields
* OTP fields

Handle:

* Standard HTML forms
* React
* Vue
* Angular
* Dynamically rendered forms
* Single-page applications
* Multi-step login flows
* Iframes where safely possible
* Password-only forms
* Username-only forms
* Email-based login
* Forms where username and password are presented on separate pages

---

# 21. Autofill field detection

Build a scoring system.

Possible signals:

```text
input type
autocomplete attribute
name
id
placeholder
aria-label
label text
surrounding text
form structure
input order
password presence
visibility
readonly state
disabled state
```

Example:

```text
username score:
+100 autocomplete=username
+80 type=email
+50 name contains username
+40 name contains email
+30 placeholder contains email
```

Do NOT rely on one heuristic.

Create:

```ts
FieldClassification
```

with:

```ts
type FieldClassification =
  | "username"
  | "password"
  | "current-password"
  | "new-password"
  | "confirmation-password"
  | "otp"
  | "unknown";
```

Include confidence.

---

# 22. MutationObserver

Modern applications dynamically modify the DOM.

Use MutationObserver carefully to detect:

* New forms
* New inputs
* SPA route changes
* Dynamically rendered authentication fields

Avoid performance problems.

Do not scan the entire document repeatedly.

Use targeted observation and debouncing.

---

# 23. Domain matching

Implement a dedicated domain-matching module.

Never perform naive string matching.

For example:

```text
github.com
```

must NOT match:

```text
evilgithub.com
github.com.evil.com
```

Support:

```text
exact URL
exact origin
registrable domain
subdomain
```

Use a robust URL parser.

Do not implement domain parsing with regular expressions alone.

Clearly define the matching rules.

---

# 24. Phishing protection

The application should be conservative.

Example:

Saved item:

```text
https://github.com
```

Current page:

```text
https://github.com
```

→ Strong match.

Current page:

```text
https://accounts.github.com
```

→ Potential subdomain match depending on configured policy.

Current page:

```text
https://github.com.evil.com
```

→ NEVER match.

Current page:

```text
https://github-login.example.com
```

→ NEVER automatically match.

Do not automatically fill credentials merely because a page "looks like" the correct website.

---

# 25. Explicit user interaction

Autofill must require explicit user action.

Do NOT silently inject credentials into pages.

Preferred flow:

```text
User focuses login field
        ↓
Extension detects candidate
        ↓
Small vault UI appears
        ↓
User clicks credential
        ↓
Extension requests secret
        ↓
Desktop authorizes request
        ↓
Credential is returned
        ↓
Extension fills fields
```

Do not autofill automatically on page load.

---

# 26. Autofill UI

Build an unobtrusive UI.

Example:

```text
┌───────────────────────────────┐
│ 🔐 Vault                      │
├───────────────────────────────┤
│ GitHub                        │
│ user@example.com              │
├───────────────────────────────┤
│ GitHub - Work                 │
│ work@example.com              │
└───────────────────────────────┘
```

The UI should not expose passwords until explicitly requested.

---

# 27. Save-login detection

Support detecting successful login attempts.

For MVP, implement conservative detection.

Potential signals:

* Form submission
* Navigation
* Password form submission
* Username/password pair entered

When a new credential is detected:

```text
Save password to Vault?
```

Do not automatically save credentials without user confirmation.

---

# 28. Password generation from extension

Allow:

```text
Generate strong password
```

and optionally insert it into a password field.

Generated passwords must use CSPRNG.

Never log generated passwords.

---

# 29. TOTP autofill

When a login item contains TOTP:

```text
Username
Password
OTP
```

the extension may request:

```text
Get TOTP for item X
```

from the desktop application.

The desktop application should generate the TOTP.

Do not expose the TOTP secret to the webpage.

Only return the current code when needed.

---

# 30. Desktop ↔ extension communication

Use Native Messaging.

The extension must NOT directly access:

* SQLite
* filesystem vault
* master password
* vault encryption key

Protocol:

```text
Extension
    │
    │ authenticated request
    ▼
Native Host
    │
    ▼
Rust Core
```

Use a strict request/response protocol.

Example:

```ts
type Request =
  | {
      type: "status";
    }
  | {
      type: "find_matches";
      origin: string;
      context: AutofillContext;
    }
  | {
      type: "fill_item";
      itemId: string;
      origin: string;
    }
  | {
      type: "get_totp";
      itemId: string;
      origin: string;
    }
  | {
      type: "lock";
    };
```

Responses must be explicit and typed.

Reject unknown commands.

Reject malformed messages.

Apply strict size limits.

---

# 31. Request authorization

Never trust:

```text
itemId
origin
field information
```

provided by the browser extension.

The Rust side must independently validate:

```text
Is the vault unlocked?
Is the item valid?
Does the item match this origin?
Is the requested operation allowed?
```

Do not rely on the extension to enforce security.

The security boundary must be enforced by the trusted side.

---

# 32. Origin binding

A request such as:

```text
fill_item(itemId, origin)
```

must cause the Rust core to independently verify:

```text
itemId → configured URL rules → current origin
```

The extension must not be able to request:

```text
fill item X
```

on an arbitrary unrelated website.

This is critical.

---

# 33. Secret minimization

Return only the secret required for the operation.

For example:

```text
find_matches()
```

should return:

```text
item ID
title
username
```

but NOT:

```text
password
TOTP secret
secure notes
```

until explicitly requested.

Similarly:

```text
fill_item()
```

should return only the fields required for that specific operation.

---

# 34. Clipboard

If implementing copy-to-clipboard:

* Provide explicit user action.
* Clear clipboard after a configurable short period where platform APIs permit.
* Never log clipboard contents.
* Never automatically copy passwords.

Document limitations because another application may read clipboard contents depending on the OS.

---

# 35. SQLite

Use SQLite for local persistence.

Do not assume SQLite encryption itself is sufficient.

The sensitive data should already be encrypted at the application layer.

Prefer:

```text
SQLite
    ↓
Encrypted vault blobs
```

rather than:

```text
SQLite
    ↓
plaintext password columns
```

Database contents should be useless to an attacker without the encryption keys.

---

# 36. Metadata minimization

Consider carefully what can remain plaintext.

Avoid storing unnecessary sensitive metadata in plaintext.

At minimum:

* Passwords encrypted
* Usernames encrypted
* TOTP secrets encrypted
* Secure notes encrypted
* URLs preferably encrypted or minimized depending on search requirements

Document any intentionally plaintext metadata.

---

# 37. Search

For MVP, prioritize security over sophisticated encrypted search.

When vault is unlocked:

```text
decrypt → search in memory
```

When locked:

```text
no secret search
```

Do not create a permanent plaintext search index.

---

# 38. Import/export

MVP may support importing from common password-manager formats.

However:

* Imports must be treated as sensitive.
* Imported plaintext files must never remain unnecessarily on disk.
* Warn users that plaintext exports are dangerous.
* Delete temporary files after processing where practical.
* Never log imported credentials.

For export:

```text
Are you sure?
Export contains all passwords in plaintext.
```

Require explicit confirmation.

---

# 39. Error handling

Never expose secrets in errors.

Bad:

```text
Failed to decrypt password hunter2
```

Good:

```text
Failed to decrypt vault item.
```

Do not stringify objects containing sensitive fields.

Be especially careful with:

```text
console.log()
Debug logs
panic messages
Rust errors
React error boundaries
```

---

# 40. Logging

Implement structured logging but create a strict security rule:

NEVER log:

* Master password
* Vault key
* Encryption keys
* Passwords
* Usernames
* TOTP secrets
* TOTP codes
* Secure-note contents
* Clipboard contents
* Full vault objects

Consider implementing a sanitizer for development logging.

---

# 41. Frontend security

The desktop UI should follow secure Tauri practices.

Do not expose unnecessary Rust commands.

Use a strict allowlist of Tauri commands.

Do not expose filesystem access broadly.

Do not enable unnecessary shell capabilities.

Do not load remote websites in the desktop application.

Avoid:

```text
eval
new Function
dangerouslySetInnerHTML
```

unless absolutely necessary.

Configure an appropriate Content Security Policy.

---

# 42. Extension CSP

Use a restrictive extension CSP.

Do not use:

```text
unsafe-eval
```

Do not use:

```text
unsafe-inline
```

unless absolutely required and justified.

Never dynamically execute webpage-provided JavaScript.

---

# 43. Browser isolation

The extension's privileged APIs must remain isolated from webpage scripts.

Do not expose extension APIs directly to the webpage.

If content scripts communicate with background scripts:

```text
Content Script
      ↓
Strictly validated message
      ↓
Background Worker
      ↓
Native Messaging
```

Validate every message.

---

# 44. Secure UI behavior

Never show passwords by default.

For password reveal:

```text
User explicitly clicks eye icon
```

For sensitive operations:

```text
User explicitly initiates action
```

Do not put passwords in browser notifications.

Do not put secrets in window titles.

Do not put secrets in URLs.

---

# 45. Testing strategy

Security testing is mandatory.

Create tests for:

## Crypto

* Encryption/decryption
* Wrong password
* Wrong key
* Tampered ciphertext
* Nonce uniqueness
* Vault corruption
* Version mismatch
* Key derivation

## Domain matching

Test:

```text
example.com
www.example.com
login.example.com
evil-example.com
example.com.evil.com
evil.com
```

## Autofill

Test:

* Normal login
* Email login
* Username login
* Password-only login
* Multi-step login
* SPA login
* Dynamically inserted fields
* Multiple forms
* Hidden fields
* Disabled fields
* iframe scenarios
* OTP fields

## Security

Test that:

* Extension cannot retrieve arbitrary vault items.
* Wrong origin cannot retrieve credentials.
* Locked vault cannot retrieve credentials.
* Unknown IPC commands are rejected.
* Malformed IPC messages are rejected.
* Invalid item IDs are rejected.
* Tampered requests are rejected.

---

# 46. Security regression tests

Create explicit tests for these attacks:

### Attack 1

Malicious page attempts:

```text
request credential for github.com
```

while running on:

```text
evil.com
```

Expected:

```text
DENIED
```

### Attack 2

Extension requests arbitrary item ID.

Expected:

```text
Only if item matches current origin.
```

### Attack 3

Vault locked.

Extension requests password.

Expected:

```text
DENIED
```

### Attack 4

Ciphertext modified.

Expected:

```text
Authentication failure.
No plaintext returned.
```

### Attack 5

Malformed native message.

Expected:

```text
Rejected safely.
No crash.
```

### Attack 6

Extremely large native message.

Expected:

```text
Rejected due to size limit.
```

### Attack 7

Page creates thousands of inputs.

Expected:

```text
No significant browser performance degradation.
```

---

# 47. Fuzzing

Where practical, fuzz:

* Vault parser
* Encrypted blob parser
* Native messaging protocol
* Domain matching
* Autofill field classification
* URL handling

Security-sensitive parsers should fail safely.

---

# 48. Dependency security

Keep dependencies minimal.

Before selecting cryptographic dependencies:

* Verify maintenance status.
* Prefer mature ecosystems.
* Prefer widely used libraries.
* Check licensing.
* Avoid abandoned crypto packages.
* Avoid unnecessary wrappers around crypto primitives.

Run dependency auditing tools.

For Rust:

```text
cargo audit
cargo deny
```

For JavaScript:

```text
npm audit
```

or the package manager equivalent.

Do not blindly accept vulnerable dependencies.

Investigate vulnerabilities before proceeding.

---

# 49. Documentation

Create:

```text
README.md
docs/architecture.md
docs/security-model.md
docs/threat-model.md
docs/autofill.md
docs/crypto.md
docs/native-messaging.md
docs/development.md
```

The security documentation should clearly state:

* What is protected
* What is not protected
* Trust boundaries
* Key hierarchy
* Encryption format
* Extension permissions
* IPC security
* Autofill security
* Threat model
* Known limitations

Do not make exaggerated claims such as:

```text
"Unhackable"
"Military-grade"
"100% secure"
```

---

# 50. Important security limitation

This is a personal password manager MVP.

Do not claim that it is equivalent to a professionally audited password manager.

The README must explicitly state:

> This software has not undergone an independent security audit and should not be considered a replacement for professionally audited password managers for high-value production use.

The goal is to build a technically serious system while maintaining honest security claims.

---

# 51. UX requirements

The application should feel modern and fast.

Desktop screens:

```text
Unlock
Vault
Login detail
Secure note
Password generator
Settings
```

Vault layout:

```text
┌─────────────────────────────────────────────┐
│ 🔐 Vault                       🔒 Locked    │
├──────────────┬──────────────────────────────┤
│ Search       │                              │
│              │ GitHub                       │
│ Logins       │ user@example.com              │
│ Secure Notes │                              │
│              │ Password                     │
│              │ •••••••••••••••              │
│              │                              │
│              │ TOTP                         │
│              │ 381 492                      │
└──────────────┴──────────────────────────────┘
```

Keep UI implementation separate from security logic.

---

# 52. Autofill UX target

The browser experience is the most important UX component.

Target flow:

```text
User visits website
        ↓
User focuses username/password field
        ↓
Extension detects matching vault item
        ↓
Small suggestion appears
        ↓
User selects item
        ↓
Username/password filled
        ↓
If TOTP exists:
        ↓
OTP suggestion appears
        ↓
User selects OTP
        ↓
OTP filled
```

The experience should feel fast enough that the user barely notices the extension.

---

# 53. Performance

Do not sacrifice security for micro-optimizations.

However:

* Avoid repeatedly decrypting the entire vault.
* Avoid repeatedly scanning the entire DOM.
* Cache only non-sensitive metadata when safe.
* Clear sensitive caches when locking.
* Debounce MutationObserver processing.
* Avoid unnecessary IPC calls.

---

# 54. Development phases

Implement in this order.

## Phase 1 — Architecture

Create:

* Monorepo
* Tauri application
* React application
* Rust core
* Extension
* Protocol package
* Test infrastructure

Do NOT implement features yet.

Verify builds.

---

## Phase 2 — Crypto core

Implement:

* Argon2id
* Random vault key
* AEAD encryption
* Decryption
* Versioned encrypted format
* Key hierarchy

Write tests before moving forward.

---

## Phase 3 — Vault

Implement:

* Create vault
* Unlock
* Lock
* Login items
* Secure notes
* Password generator
* Search
* Auto-lock

All tests must pass.

---

## Phase 4 — Native Messaging

Implement:

```text
Extension
   ↕
Native Messaging
   ↕
Rust
```

Start with:

```text
status
lock
```

Then add:

```text
find_matches
get_item
get_totp
```

---

## Phase 5 — Autofill

Implement:

1. URL parsing
2. Domain matching
3. Field detection
4. Candidate detection
5. Suggestion UI
6. Explicit fill
7. Save login
8. Password generation
9. TOTP

Test each separately.

---

## Phase 6 — Security hardening

Perform:

* Dependency audit
* Permission audit
* IPC fuzzing
* Domain matching tests
* Autofill attack tests
* Secret logging audit
* CSP audit
* Tauri capability audit
* Memory/cache review
* Error-message audit

---

# 55. Definition of done

The MVP is NOT complete simply because the UI works.

It is complete only when:

### Desktop

* [ ] Vault creation works
* [ ] Vault unlock works
* [ ] Vault lock works
* [ ] Auto-lock works
* [ ] Login items work
* [ ] Secure notes work
* [ ] Password generator works
* [ ] TOTP works
* [ ] Search works
* [ ] Data remains encrypted at rest

### Extension

* [ ] Extension installs
* [ ] Extension communicates through Native Messaging
* [ ] Login detection works
* [ ] Domain matching works
* [ ] Credential suggestions work
* [ ] Explicit autofill works
* [ ] Password generation works
* [ ] TOTP autofill works
* [ ] Save-login flow works

### Security

* [ ] Master password is never persisted
* [ ] Secrets are never logged
* [ ] Passwords are never stored plaintext
* [ ] TOTP secrets are encrypted
* [ ] Secure notes are encrypted
* [ ] Locked vault refuses secret requests
* [ ] Origin validation occurs in Rust
* [ ] Malicious origins cannot request credentials
* [ ] Native messages are validated
* [ ] Extension permissions are minimized
* [ ] CSP is restrictive
* [ ] Security tests pass
* [ ] Dependency audit completed
* [ ] Threat model documented
* [ ] Known limitations documented

---

# 56. Critical engineering rules

Throughout development, follow these rules:

1. **Never invent cryptography.**
2. **Never log secrets.**
3. **Never trust webpage input.**
4. **Never trust the extension to enforce security.**
5. **Validate security-sensitive requests in Rust.**
6. **Never autofill without explicit user interaction.**
7. **Never send passwords unless they are actually needed.**
8. **Never store plaintext vault data unnecessarily.**
9. **Never use insecure randomness.**
10. **Never use naive domain matching.**
11. **Never expose the master password to the extension.**
12. **Never expose vault keys to JavaScript.**
13. **Never use `eval` or equivalent dynamic code execution.**
14. **Never make security claims that have not been verified.**
15. **Prefer a smaller secure implementation over a larger feature-rich insecure one.**

---

# 57. Claude Code workflow

Do not generate the entire application blindly in one pass.

Work incrementally.

At the beginning:

1. Inspect the repository.
2. Determine existing tooling.
3. Propose the final architecture.
4. Identify security-critical components.
5. Create the threat model.
6. Create the security model.
7. Create the architecture documentation.
8. Then begin implementation.

After every major phase:

1. Run tests.
2. Run type checking.
3. Run Rust checks.
4. Run dependency/security audits where applicable.
5. Review the implementation for secret leakage.
6. Review the trust boundaries.
7. Update documentation.

Before declaring the MVP finished, perform a dedicated **security review of your own implementation**.

Create:

```text
docs/security-review.md
```

containing:

* Findings
* Severity
* Affected component
* Attack scenario
* Mitigation
* Remaining limitations

Do not hide or ignore security problems simply because they are inconvenient to fix.

---

# Final objective

Build a small, elegant, secure password manager — local-first in the sense
that keys and plaintext never leave the device, with a server the user owns
holding only ciphertext — that provides an experience similar to:

```text
1Password
   ↓
excellent browser integration
   ↓
excellent autofill
   ↓
secure vault
   ↓
TOTP
   ↓
secure notes
```

but without:

```text
cloud
accounts
subscription
server
mobile
sharing
```

for the MVP.

The application should feel like a **real password manager**, not a CRUD application with an encrypted database.

Prioritize:

```text
Security
   >
Correctness
   >
Autofill quality
   >
Reliability
   >
UX
   >
Features
```

Do not add unnecessary features until the security model, vault encryption, IPC and autofill architecture are solid.

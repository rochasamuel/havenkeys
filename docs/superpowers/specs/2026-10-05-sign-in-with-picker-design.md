# Sign in with: pick a saved account, and five more providers — Design

Date: 2026-10-05. Status: approved in conversation.
Amends `2026-09-28-sign-in-with-design.md` (§3.1 providers, §5.1 button
recognition, §8 desktop) and `2026-09-29-sign-in-with-provider-login-design.md`
(nothing in its run changes; §2's "derived by account" decision stands).

> This software has not undergone an independent security audit.

## 1. Goal

Today the login editor asks for a provider (a dropdown) and then for the
account as free text. The detail view later tries to find the provider's own
login by comparing that text with usernames, in TypeScript, against a domain
table of its own. A typo or a different form of the username (an email
instead of a GitHub handle) silently breaks the link.

1Password instead shows one list: each provider, and under it the logins
the vault already holds for that provider. HavenKeys should do the same, and
support five more providers: **Facebook, Discord, X, LinkedIn, GitLab**.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Several providers on one login | **No.** One provider per login, as today. Separate accounts are separate items | A list of providers (every run, menu row and import would have to choose one) |
| How the editor links to the provider's login | **Pick from a list, copy the username** into `signInWith.account`. The vault format is unchanged | Storing the provider login's item id (dangles on delete, a second rule Rust must trust; rejected again for the reasons in the 2026-09-29 spec §2) |
| Who decides which logins belong to a provider | **Rust**, with the rule it already uses for runs and the save prompt (`find_matches` over the provider's origins) | The desktop's `PROVIDER_DOMAINS` table in `lib/sso.ts` (removed) |
| Detail link when several logins match | **None**, plus a note. Exactly one, as the run requires | First match (today's behaviour; could open a login the run would never use) |
| New providers | Facebook, Discord, X, LinkedIn, GitLab: each signs in on fixed hosts | Amazon (regional hosts), Slack (per-workspace subdomains): later, separately |
| What a run does on a new provider's page | **Nothing new.** Like GitHub today: fill that provider's login if a login form appears; stop at any consent screen | Provider-specific steps |

## 3. Providers (Rust, `crates/havenkeys-core/src/sso.rs`)

`SsoProvider` gains five variants. Serialized lowercase, as today.

| Variant | Wire | `name()` | `origins()` (exact) |
|---|---|---|---|
| `Facebook` | `facebook` | Facebook | `https://www.facebook.com`, `https://m.facebook.com` |
| `Discord` | `discord` | Discord | `https://discord.com` |
| `X` | `x` | X | `https://x.com`, `https://twitter.com`, `https://api.x.com`, `https://api.twitter.com` |
| `Linkedin` | `linkedin` | LinkedIn | `https://www.linkedin.com` |
| `Gitlab` | `gitlab` | GitLab | `https://gitlab.com` |

`ALL` lists the nine in alphabetical order of `name()`: Apple, Discord,
Facebook, GitHub, GitLab, Google, LinkedIn, Microsoft, X.

The X `api.*` hosts serve the older OAuth 1.0a "Sign in with Twitter"
(`/oauth/authenticate`). During implementation, open each provider's real
OAuth flow and keep only the origins a sign-in actually passes through; a
host that is not used is removed, not kept "just in case". Record the
checked flows in the plan.

`from_name` (1Password import) picks the new names up through `name()`. It
also accepts `"Twitter"` for X.

Mirror: `packages/protocol/src/sso.ts` (`SsoProvider`, `SSO_PROVIDERS`),
kept equal by `sso-parity.test.ts`. `BUTTON_FRAMES` stays Google-only.

### 3.1 Compatibility

The overview is decoded with `deny_unknown_fields` on `SignInWith` and a
closed `SsoProvider` enum. An app built before this change cannot decode a
login whose provider is one of the five; it counts that item as unreadable
(the existing damaged/unreadable path, nothing is deleted) until updated.
Desktop, extension and Android ship together in one release. The release
notes say so. No vault migration: existing values keep their meaning.

## 4. Rust commands

### 4.1 `sso_accounts(provider) -> Vec<SsoAccount>`

```rust
pub struct SsoAccount { pub id: Uuid, pub title: String, pub username: String }
```

The logins `find_matches` returns for each of the provider's origins
(`{origin}/`, no top URL), deduplicated by id, with a non-empty username,
sorted by title then username, at most `MAX_SSO_PICKER_ACCOUNTS = 50`. No
secrets. `provider_accounts` (the save prompt's list) is rewritten on top of
this so the two cannot drift.

Tauri command `sso_accounts`, added to the allowlist and capability file.
Locked vault → `locked`, as for every read.

### 4.2 `provider_login(item_id) -> ProviderLogin`

```rust
pub enum ProviderLogin { One(Uuid), None, Several }
```

For a login with `signInWith` and an account: the logins of
`sso_accounts(provider)` whose normalized username (`normalize_username`)
equals the normalized account, excluding the item itself: exactly one →
`One(id)`, zero → `None`, more → `Several`. A login without `signInWith`
or without an account → `None`. This is the same rule the extension's run
applies on the provider page (2026-09-29 spec §3 step 3.2).

Tauri command `provider_login`. The desktop's `providerLogin()` and
`PROVIDER_DOMAINS` in `apps/desktop/src/lib/sso.ts` are removed.

## 5. Extension: recognizing the new buttons (`autofill/sso.ts`)

`NAMES` gains:

| Provider | Names (after `normalize()`) |
|---|---|
| facebook | `facebook` |
| discord | `discord` |
| x | `twitter`; `x` **only with a joiner** |
| linkedin | `linkedin`, `linked in` |
| gitlab | `gitlab`, `git lab` |

**X with no joiner never counts.** A bare "X" or "×" is a close button on
countless pages. `providerOf` gets a per-name flag `joinerOnly`: such a
name scores 80 with a joiner ("sign in with x", "continuar com o x") and 0
otherwise. Neither the bare-label score nor the `href` bonus alone can
lift it. "Twitter" behaves like the other names.

`NEGATIVE` gains `share`, `follow`, `compartilhar`, `seguir` ("Share on
Facebook", "Follow us on X" are not sign-in buttons).

Nothing else in the run changes: the press, the provider-page steps, the
consent check, the "Use another account" phrases and the 2-minute window
are provider-independent already. The chooser-row match by email stays;
for providers whose account is a handle (GitHub, GitLab, Discord, X) the
run continues at the login form, as GitHub does today.

Provider icons: the extension's menu and save balloon get the five marks
(inline SVG, single path each, from Simple Icons, CC0), added to
`menu/icons.ts` next to the existing four and drawn the same way.

## 6. Desktop

### 6.1 Editor picker (`ItemEditor.tsx`, new `components/SsoPicker.tsx`)

The provider `<select>` and the account field become one control:

```text
Sign in with  [ G  Google · samuelsilv.rocha@gmail.com      ▾ ]
              Account  [ samuelsilv.rocha@gmail.com ]
```

Opening it shows a searchable list (a text box on top filters by provider
name, title and username), grouped in `ALL` order:

```text
[🔍 Search                        ]
  Apple
  Discord
  Facebook
  GitHub
    Github · rochasamuel
  GitLab
  Google
    google.com · samuelsilv.rocha@gmail.com
  …
  None
```

* A provider row: sets the provider; the account is left as it was if the
  provider did not change, else emptied.
* A saved-login row (indented, provider icon, title · username): sets the
  provider and copies that login's username into the account.
* "None": clears `signInWith`.
* Keyboard: arrows move, Enter picks, Escape closes; the list is a
  `listbox` with `aria-activedescendant`.

The account stays an editable text field under the picker, for an account
not saved in the vault. Saving is unchanged (`signInWith: { provider,
account }`, validated in Rust by `clean_sign_in_with`).

The saved logins come from `sso_accounts`, one call per provider when the
list first opens (nine calls, overviews only), cached for the editor's
lifetime.

### 6.2 Detail view (`ItemDetail.tsx`)

The "Sign in with" row asks `provider_login(item.id)` on mount. A result →
the arrow button that opens it (unchanged). `None` with an account set → a
muted line under the row: "No saved Google login for this account" on
`None`, or "Several saved Google logins match this account" on `Several`.

The `items` prop and the `allItems` list added to `VaultScreen` for it are
kept only for the open item's lookup (that fix stands on its own).

### 6.3 Icons

`components/ProviderIcon.tsx` gains the five marks (same source and
treatment as the extension's).

## 7. Android

No screen shows or edits the provider today. Only the Rust enum change
reaches it (§3.1). `crates/havenkeys-mobile` round-trips `signInWith`
unchanged when editing other fields; a test covers a `discord` login.

## 8. Security

* Which logins belong to a provider, which one a run fills, and where a run
  may continue are all decided in Rust from fixed origin lists. The picker
  only helps the user type the account; a typed or edited account goes
  through the same `clean_sign_in_with` validation as today.
* `sso_accounts` and `provider_login` return titles, usernames and ids
  only, never passwords, TOTP or notes (§33), to the desktop UI, never to
  the extension.
* The new origins are exact strings; look-alike hosts are tested.
* "X" can never be recognized from a bare label (§5).

## 9. Translations

New strings in `en` and `pt-BR` (desktop and extension): picker search
placeholder, "None", the two detail notes, and the five provider names
(not translated).

## 10. Testing

Rust (`sso.rs`, `vault.rs`):
* each new provider's origins; `http://`, `evil` suffix/prefix hosts, and
  `facebook.com` without `www.` are rejected;
* `from_name("Twitter") == Some(X)`, `from_name("GitLab")`;
* `sso_accounts`: returns only logins matching the provider page, no
  duplicates, skips empty usernames, cap at 50, `locked` when locked;
* `provider_login`: one / none / several; ignores the item itself;
  case and whitespace insensitive; `provider_accounts` unchanged results.

Protocol: `sso-parity.test.ts` covers the nine.

Extension (`autofill/sso.test.ts`):
* "Continue with Facebook", "Entrar com o LinkedIn", "Sign in with GitLab",
  "Log in with Discord", "Sign in with X", "Continuar com o X" recognized;
* a bare "X", "×" and a close button with `aria-label="X"` never recognized,
  with and without a login field on the page;
* "Share on Facebook", "Follow us on X" never recognized.

Desktop:
* `SsoPicker`: groups and order, filter, picking a login copies its
  username, picking a provider keeps or clears the account, "None", keyboard;
* `ItemDetail`: arrow on `One`, each note on `None` / `Several`.

## 11. Docs

`docs/autofill.md` and `docs/security-model.md`: the provider table with
origins; the `sso_accounts` / `provider_login` commands in
`docs/architecture.md`'s Tauri command list.

## 12. Out of scope

* More than one provider per login.
* Amazon, Slack and any provider without fixed sign-in hosts.
* Storing a link to the provider login's item id.
* An Android editor for "Sign in with".

# Sign in with: saved-account picker and five more providers — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The login editor picks a "Sign in with" account from the vault's own provider logins, Rust decides which login is a provider's, and Facebook, Discord, X, LinkedIn and GitLab join Google, Microsoft, GitHub and Apple.

**Architecture:** The provider list stays a closed Rust enum (`havenkeys-core/src/sso.rs`) mirrored in the wire protocol crate, the TS protocol package and the icon package. Two new read-only core methods (`sso_accounts`, `provider_login`) reuse `find_matches` over the provider's fixed origins and are exposed as Tauri commands; the desktop's own domain table is deleted. The editor gets a `SsoPicker` combobox built on a pure row model in `lib/sso.ts`.

**Tech Stack:** Rust (havenkeys-core, havenkeys-protocol, havenkeys-bridge, havenkeys-mobile, Tauri 2), TypeScript, React 19, vitest + jsdom, pnpm workspaces.

**Spec:** `docs/superpowers/specs/2026-10-05-sign-in-with-picker-design.md`

## Global Constraints

- One provider per login. The vault format (`SignInWith { provider, account }`) is unchanged; no item id is stored.
- New wire names, exactly: `facebook`, `discord`, `x`, `linkedin`, `gitlab`. Display names: Facebook, Discord, X, LinkedIn, GitLab.
- `SsoProvider::ALL` and the desktop `PROVIDER_ORDER` are alphabetical by display name: Apple, Discord, Facebook, GitHub, GitLab, Google, LinkedIn, Microsoft, X.
- Origins are exact strings: Facebook `https://www.facebook.com`, `https://m.facebook.com`; Discord `https://discord.com`; X `https://x.com`, `https://twitter.com`, `https://api.x.com`, `https://api.twitter.com` (subject to Task 1 Step 1); LinkedIn `https://www.linkedin.com`; GitLab `https://gitlab.com`.
- `MAX_SSO_PICKER_ACCOUNTS = 50`. `sso_accounts`/`provider_login` return ids, titles and usernames only — never passwords, TOTP, notes.
- "X" is never recognized from a bare label; only "<sign-in verb> … <joiner> x" at the end of the label.
- Never log usernames or accounts (`Debug` impls hide them). No `innerHTML`/`dangerouslySetInnerHTML`.
- No Claude co-author trailer on commits (owner's rule). Commit after each task.
- Strings in both `en` and `pt-BR`; provider names are not translated.

## Review Focus

- A label such as "acme.com X" or "Help · X" (joiner-like word before a bare "x", no sign-in verb) must not be recognized as an X button → test in Task 2.
- The provider login is deleted after the site login was linked → the detail view shows the "No saved … login" note, never a stale arrow or an error → test in Task 3 (`provider_login` after delete) and Task 5.
- The vault locks while the picker is open, so `sso_accounts` rejects with `locked` → the picker still lists the nine providers, without saved logins, and shows no error toast → test in Task 6.
- Editing an existing login whose account matches no saved login → the trigger shows the stored provider · account and saving without touching the picker keeps them unchanged → test in Task 6.
- A filter that matches nothing → the list shows "No matches", Enter does nothing, Escape closes and keeps the previous value → test in Task 6.

---

### Task 1: Nine providers in every provider table

**Files:**
- Modify: `crates/havenkeys-core/src/sso.rs` (enum, `ALL`, `name`, `origins`, `from_name`, tests)
- Modify: `crates/havenkeys-protocol/src/message.rs:1031-1039` (wire enum)
- Modify: `crates/havenkeys-bridge/src/convert.rs:36-43,107-114` (conversions)
- Modify: `crates/havenkeys-mobile/src/edit.rs:846` and its assertion (Discord round-trip)
- Modify: `packages/protocol/src/sso.ts` (type + table)
- Modify: `packages/ui/src/provider-icons.ts`, `packages/ui/src/provider-icons.test.ts`
- Modify: `apps/desktop/src/lib/types.ts:22`, `apps/desktop/src/lib/sso.ts:7-8`
- Test: `crates/havenkeys-core/src/sso.rs` (unit), `packages/protocol/src/sso-parity.test.ts` (unchanged, must pass), `packages/protocol/src/sso.test.ts` (new)

**Interfaces:**
- Produces: `SsoProvider::{Apple, Discord, Facebook, Github, Gitlab, Google, Linkedin, Microsoft, X}` (Rust core and protocol crate); TS `SsoProvider = "apple" | "discord" | "facebook" | "github" | "gitlab" | "google" | "linkedin" | "microsoft" | "x"` in `@havenkeys/protocol` and `apps/desktop/src/lib/types.ts`; `PROVIDER_ICONS` keyed by all nine; desktop `PROVIDER_ORDER` and `PROVIDER_NAMES` with all nine.

- [ ] **Step 1: Confirm the X origins**

Use WebFetch on X's developer documentation for OAuth 2.0 authorization code flow and for OAuth 1.0a "Sign in with X" (`/oauth/authenticate`). Confirm the hosts of the authorize/authenticate URLs and of the login page a signed-out user is sent to. Keep `https://x.com` and `https://twitter.com` (OAuth 2.0 authorize, login page). Keep `https://api.x.com` / `https://api.twitter.com` only if the docs show `/oauth/authenticate` or `/oauth/authorize` served on them today; remove any that are not. Do the same quick check for Facebook (`www.facebook.com/vXX.X/dialog/oauth`), Discord (`discord.com/oauth2/authorize`), LinkedIn (`www.linkedin.com/oauth/v2/authorization`), GitLab (`gitlab.com/oauth/authorize`). Write the confirmed list as a comment block above `origins()` (one line per provider: "<host>: <path seen in docs>"). If a check changes the list, use the changed list everywhere below.

- [ ] **Step 2: Write the failing Rust tests**

In `crates/havenkeys-core/src/sso.rs`, add to `mod tests`:

```rust
    #[test]
    fn new_providers_wire_names_and_origins() {
        for (p, wire, name) in [
            (SsoProvider::Facebook, "facebook", "Facebook"),
            (SsoProvider::Discord, "discord", "Discord"),
            (SsoProvider::X, "x", "X"),
            (SsoProvider::Linkedin, "linkedin", "LinkedIn"),
            (SsoProvider::Gitlab, "gitlab", "GitLab"),
        ] {
            assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{wire}\""));
            assert_eq!(p.name(), name);
            assert_eq!(SsoProvider::from_name(name), Some(p));
        }
        assert!(SsoProvider::Facebook.allows_origin("https://www.facebook.com"));
        assert!(!SsoProvider::Facebook.allows_origin("https://facebook.com"));
        assert!(!SsoProvider::Facebook.allows_origin("http://www.facebook.com"));
        assert!(!SsoProvider::Facebook.allows_origin("https://www.facebook.com.evil.com"));
        assert!(SsoProvider::Discord.allows_origin("https://discord.com"));
        assert!(!SsoProvider::Discord.allows_origin("https://evildiscord.com"));
        assert!(SsoProvider::X.allows_origin("https://x.com"));
        assert!(SsoProvider::X.allows_origin("https://twitter.com"));
        assert!(!SsoProvider::X.allows_origin("https://x.com.evil.com"));
        assert!(SsoProvider::Linkedin.allows_origin("https://www.linkedin.com"));
        assert!(!SsoProvider::Linkedin.allows_origin("https://linkedin.com.evil.com"));
        assert!(SsoProvider::Gitlab.allows_origin("https://gitlab.com"));
        assert!(!SsoProvider::Gitlab.allows_origin("https://gitlab.example.com"));
        assert_eq!(SsoProvider::from_name(" twitter "), Some(SsoProvider::X));
    }

    #[test]
    fn all_is_alphabetical_by_name() {
        let names: Vec<&str> = SsoProvider::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(
            names,
            ["Apple", "Discord", "Facebook", "GitHub", "GitLab", "Google", "LinkedIn", "Microsoft", "X"]
        );
    }
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p havenkeys-core --lib sso::`
Expected: compile error, `no variant named Facebook`.

- [ ] **Step 4: Implement the core enum**

Replace the enum and `impl SsoProvider` head in `sso.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SsoProvider {
    Google,
    Microsoft,
    Github,
    Apple,
    Facebook,
    Discord,
    X,
    Linkedin,
    Gitlab,
}

impl SsoProvider {
    /// Alphabetical by display name: the order the desktop lists them in.
    pub const ALL: [SsoProvider; 9] = [
        Self::Apple,
        Self::Discord,
        Self::Facebook,
        Self::Github,
        Self::Gitlab,
        Self::Google,
        Self::Linkedin,
        Self::Microsoft,
        Self::X,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::Microsoft => "Microsoft",
            Self::Github => "GitHub",
            Self::Apple => "Apple",
            Self::Facebook => "Facebook",
            Self::Discord => "Discord",
            Self::X => "X",
            Self::Linkedin => "LinkedIn",
            Self::Gitlab => "GitLab",
        }
    }

    /// Exact origins (scheme + host, no port, no trailing slash).
    pub fn origins(self) -> &'static [&'static str] {
        match self {
            Self::Google => &["https://accounts.google.com"],
            Self::Microsoft => &[
                "https://login.microsoftonline.com",
                "https://login.live.com",
            ],
            Self::Github => &["https://github.com"],
            Self::Apple => &["https://appleid.apple.com"],
            Self::Facebook => &["https://www.facebook.com", "https://m.facebook.com"],
            Self::Discord => &["https://discord.com"],
            Self::X => &[
                "https://x.com",
                "https://twitter.com",
                "https://api.x.com",
                "https://api.twitter.com",
            ],
            Self::Linkedin => &["https://www.linkedin.com"],
            Self::Gitlab => &["https://gitlab.com"],
        }
    }
```

Keep `allows_origin` as is. Replace `from_name`:

```rust
    /// A provider named in an export (`"Google"`, `"github"`, `"Twitter"`),
    /// case-insensitive.
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim();
        if n.eq_ignore_ascii_case("twitter") {
            return Some(Self::X);
        }
        Self::ALL
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(n))
    }
```

Note: the parity test parses `Self::<Variant> => "..."` / `=> &[...]` arms inside `name(self)` and `origins(self)`; keep that shape (one arm per variant, string literals only).

- [ ] **Step 5: Wire enum and bridge conversions**

`crates/havenkeys-protocol/src/message.rs`, the `SsoProvider` enum becomes:

```rust
pub enum SsoProvider {
    Google,
    Microsoft,
    Github,
    Apple,
    Facebook,
    Discord,
    X,
    Linkedin,
    Gitlab,
}
```

`crates/havenkeys-bridge/src/convert.rs`, add to `wire_provider`:

```rust
        CoreProvider::Facebook => WireProvider::Facebook,
        CoreProvider::Discord => WireProvider::Discord,
        CoreProvider::X => WireProvider::X,
        CoreProvider::Linkedin => WireProvider::Linkedin,
        CoreProvider::Gitlab => WireProvider::Gitlab,
```

and to `core_provider`:

```rust
        WireProvider::Facebook => CoreProvider::Facebook,
        WireProvider::Discord => CoreProvider::Discord,
        WireProvider::X => CoreProvider::X,
        WireProvider::Linkedin => CoreProvider::Linkedin,
        WireProvider::Gitlab => CoreProvider::Gitlab,
```

- [ ] **Step 6: Mobile round-trip uses a new provider**

In `crates/havenkeys-mobile/src/edit.rs`, test `an_edit_keeps_what_the_phone_does_not_edit`: change `provider: SsoProvider::Google,` (line 846) to `provider: SsoProvider::Discord,`. Next to the existing assertion on `o.sign_in_with.as_ref().unwrap().account` add:

```rust
        assert_eq!(o.sign_in_with.as_ref().unwrap().provider, SsoProvider::Discord);
```

- [ ] **Step 7: Run the Rust tests**

Run: `cargo test -p havenkeys-core -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-mobile`
Expected: all pass.

- [ ] **Step 8: Write the failing TS tests**

Create `packages/protocol/src/sso.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { isSsoProvider, providersForOrigin, SSO_PROVIDER_IDS } from "./sso";

describe("sso providers", () => {
  it("accepts the nine wire names and nothing else", () => {
    expect([...SSO_PROVIDER_IDS].sort()).toEqual(["apple", "discord", "facebook", "github", "gitlab", "google", "linkedin", "microsoft", "x"]);
    for (const p of ["x", "linkedin", "gitlab", "discord", "facebook"]) expect(isSsoProvider(p)).toBe(true);
    for (const p of ["X", "twitter", "okta", "", "__proto__", 1, null]) expect(isSsoProvider(p)).toBe(false);
  });
  it("maps exact origins only", () => {
    expect(providersForOrigin("https://twitter.com")).toEqual(["x"]);
    expect(providersForOrigin("https://www.facebook.com")).toEqual(["facebook"]);
    expect(providersForOrigin("https://facebook.com")).toEqual([]);
    expect(providersForOrigin("https://gitlab.com.evil.com")).toEqual([]);
  });
});
```

In `packages/ui/src/provider-icons.test.ts` change the expected keys to:

```ts
    expect(Object.keys(PROVIDER_ICONS).sort()).toEqual(["apple", "discord", "facebook", "github", "gitlab", "google", "linkedin", "microsoft", "x"]);
```

Run: `pnpm --filter @havenkeys/protocol test && pnpm --filter @havenkeys/ui test`
Expected: FAIL (parity test: key sets differ; new tests: missing providers; icons: missing keys).

- [ ] **Step 9: TS protocol table**

`packages/protocol/src/sso.ts`:

```ts
export type SsoProvider = "google" | "microsoft" | "github" | "apple" | "facebook" | "discord" | "x" | "linkedin" | "gitlab";

export const SSO_PROVIDERS: Readonly<Record<SsoProvider, { name: string; origins: readonly string[] }>> = {
  google: { name: "Google", origins: ["https://accounts.google.com"] },
  microsoft: { name: "Microsoft", origins: ["https://login.microsoftonline.com", "https://login.live.com"] },
  github: { name: "GitHub", origins: ["https://github.com"] },
  apple: { name: "Apple", origins: ["https://appleid.apple.com"] },
  facebook: { name: "Facebook", origins: ["https://www.facebook.com", "https://m.facebook.com"] },
  discord: { name: "Discord", origins: ["https://discord.com"] },
  x: { name: "X", origins: ["https://x.com", "https://twitter.com", "https://api.x.com", "https://api.twitter.com"] },
  linkedin: { name: "LinkedIn", origins: ["https://www.linkedin.com"] },
  gitlab: { name: "GitLab", origins: ["https://gitlab.com"] },
};
```

(Origins in the same order as Rust; the parity test compares arrays.)

- [ ] **Step 10: Icons (Simple Icons, CC0-1.0, v13.21.0)**

In `packages/ui/src/provider-icons.ts` change the record's key type to `"google" | "microsoft" | "github" | "apple" | "facebook" | "discord" | "x" | "linkedin" | "gitlab"`, add a line to the header comment: `// Facebook, Discord, X, LinkedIn, GitLab: Simple Icons 13.21.0 (CC0-1.0).`, and add these entries after `apple`:

```ts
  facebook: {
    viewBox: "0 0 24 24",
    shapes: [
      {
        fill: "#0866FF",
        d: "M9.101 23.691v-7.98H6.627v-3.667h2.474v-1.58c0-4.085 1.848-5.978 5.858-5.978.401 0 .955.042 1.468.103a8.68 8.68 0 0 1 1.141.195v3.325a8.623 8.623 0 0 0-.653-.036 26.805 26.805 0 0 0-.733-.009c-.707 0-1.259.096-1.675.309a1.686 1.686 0 0 0-.679.622c-.258.42-.374.995-.374 1.752v1.297h3.919l-.386 2.103-.287 1.564h-3.246v8.245C19.396 23.238 24 18.179 24 12.044c0-6.627-5.373-12-12-12s-12 5.373-12 12c0 5.628 3.874 10.35 9.101 11.647Z",
      },
    ],
  },
  discord: {
    viewBox: "0 0 24 24",
    shapes: [
      {
        fill: "#5865F2",
        d: "M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z",
      },
    ],
  },
  x: {
    viewBox: "0 0 24 24",
    shapes: [
      {
        fill: "currentColor",
        d: "M18.901 1.153h3.68l-8.04 9.19L24 22.846h-7.406l-5.8-7.584-6.638 7.584H.474l8.6-9.83L0 1.154h7.594l5.243 6.932ZM17.61 20.644h2.039L6.486 3.24H4.298Z",
      },
    ],
  },
  linkedin: {
    viewBox: "0 0 24 24",
    shapes: [
      {
        fill: "#0A66C2",
        d: "M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z",
      },
    ],
  },
  gitlab: {
    viewBox: "0 0 24 24",
    shapes: [
      {
        fill: "#FC6D26",
        d: "m23.6004 9.5927-.0337-.0862L20.3.9814a.851.851 0 0 0-.3362-.405.8748.8748 0 0 0-.9997.0539.8748.8748 0 0 0-.29.4399l-2.2055 6.748H7.5375l-2.2057-6.748a.8573.8573 0 0 0-.29-.4412.8748.8748 0 0 0-.9997-.0537.8585.8585 0 0 0-.3362.4049L.4332 9.5015l-.0325.0862a6.0657 6.0657 0 0 0 2.0119 7.0105l.0113.0087.03.0213 4.976 3.7264 2.462 1.8633 1.4995 1.1321a1.0085 1.0085 0 0 0 1.2197 0l1.4995-1.1321 2.4619-1.8633 5.006-3.7489.0125-.01a6.0682 6.0682 0 0 0 2.0094-7.003z",
      },
    ],
  },
```

- [ ] **Step 11: Desktop provider type and order**

`apps/desktop/src/lib/types.ts:22`:

```ts
export type SsoProvider = "google" | "microsoft" | "github" | "apple" | "facebook" | "discord" | "x" | "linkedin" | "gitlab";
```

`apps/desktop/src/lib/sso.ts`:

```ts
/** Alphabetical by name, as havenkeys-core's `SsoProvider::ALL`. */
export const PROVIDER_ORDER: SsoProvider[] = ["apple", "discord", "facebook", "github", "gitlab", "google", "linkedin", "microsoft", "x"];
export const PROVIDER_NAMES: Record<SsoProvider, string> = {
  apple: "Apple",
  discord: "Discord",
  facebook: "Facebook",
  github: "GitHub",
  gitlab: "GitLab",
  google: "Google",
  linkedin: "LinkedIn",
  microsoft: "Microsoft",
  x: "X",
};
```

`PROVIDER_DOMAINS` in the same file must type-check too; add placeholder-free entries so it compiles until Task 5 deletes it:

```ts
  facebook: ["facebook.com"],
  discord: ["discord.com"],
  x: ["x.com", "twitter.com"],
  linkedin: ["linkedin.com"],
  gitlab: ["gitlab.com"],
```

- [ ] **Step 12: Run everything this touched**

Run: `pnpm --filter @havenkeys/protocol test && pnpm --filter @havenkeys/ui test && pnpm -r typecheck && pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/extension test`
Expected: all pass (the extension's `NAMES` record is `Record<SsoProvider, …>` and will fail typecheck — if so, add the five keys with the exact names from Task 2 Step 3 now and leave the joiner rule to Task 2).

- [ ] **Step 13: Commit**

```bash
git add crates/havenkeys-core/src/sso.rs crates/havenkeys-protocol/src/message.rs crates/havenkeys-bridge/src/convert.rs crates/havenkeys-mobile/src/edit.rs packages/protocol/src/sso.ts packages/protocol/src/sso.test.ts packages/ui/src/provider-icons.ts packages/ui/src/provider-icons.test.ts apps/desktop/src/lib/types.ts apps/desktop/src/lib/sso.ts apps/extension/src/autofill/sso.ts
git commit -m "feat(sso): Facebook, Discord, X, LinkedIn and GitLab as sign-in providers"
```

---

### Task 2: Recognizing the new providers' buttons

**Files:**
- Modify: `apps/extension/src/autofill/sso.ts:20-31` (`NAMES`, `NEGATIVE`), `providerOf` (lines ~104-118)
- Test: `apps/extension/src/autofill/sso.test.ts`

**Interfaces:**
- Consumes: TS `SsoProvider` with nine members (Task 1).
- Produces: `providerOf`, `findProviderButtons`, `providerButton` recognize the five; signatures unchanged.

- [ ] **Step 1: Write the failing tests**

Add to `describe("provider buttons", …)` in `apps/extension/src/autofill/sso.test.ts`:

```ts
  it("recognises the new providers", () => {
    const root = page(`
      <button>Continue with Facebook</button>
      <button>Log in with Discord</button>
      <button>Entrar com o LinkedIn</button>
      <a href="https://gitlab.com/oauth/authorize?client_id=x">Sign in with GitLab</a>
      <button>Sign in with X</button>`);
    expect([...findProviderButtons(root, env).keys()].sort()).toEqual(["discord", "facebook", "gitlab", "linkedin", "x"]);
    expect(findProviderButtons(page(`<button>Continuar com o X</button>`), env).has("x")).toBe(true);
    expect(findProviderButtons(page(`<button>Sign in with Twitter</button>`), env).has("x")).toBe(true);
  });
  it("never takes a bare X for the X provider", () => {
    for (const html of [
      `<input type="email"><button>X</button><button>Google</button>`,
      `<input type="email"><button aria-label="X"></button>`,
      `<input type="email"><button>×</button>`,
      `<input type="email"><a href="https://x.com/acme">X</a><button>GitHub</button>`,
      `<input type="email"><button>acme.com X</button>`,
      `<input type="email"><button>Help with x</button>`,
    ]) {
      expect(findProviderButtons(page(html), env).has("x"), html).toBe(false);
    }
  });
  it("ignores share and follow buttons", () => {
    const root = page(`
      <input type="email">
      <button>Share on Facebook</button>
      <a href="https://x.com/acme">Follow us on X</a>
      <button>Compartilhar no LinkedIn</button>
      <button>Seguir no Discord</button>`);
    expect(findProviderButtons(root, env).size).toBe(0);
  });
```

- [ ] **Step 2: Run them to see them fail**

Run: `pnpm --filter @havenkeys/extension exec vitest run src/autofill/sso.test.ts`
Expected: FAIL in all three new tests (the bare-X and share cases may pass partially; "recognises the new providers" fails).

- [ ] **Step 3: Implement**

In `apps/extension/src/autofill/sso.ts` replace `NAMES` and extend `NEGATIVE`:

```ts
// `normalize()` splits camelCase, so "GitHub" becomes "git hub"; "Github" or
// "github" do not split. Both spellings are accepted.
const NAMES: Record<SsoProvider, readonly string[]> = {
  google: ["google"],
  microsoft: ["microsoft"],
  github: ["github", "git hub"],
  apple: ["apple"],
  facebook: ["facebook"],
  discord: ["discord"],
  x: ["twitter", "x"],
  linkedin: ["linkedin", "linked in"],
  gitlab: ["gitlab", "git lab"],
};
/**
 * Names that are also common words or glyphs ("X" closes dialogs). They
 * count only at the end of a sign-in label, right after a joiner:
 * "Sign in with X", "Continuar com o X". Never bare, never from a link alone.
 */
const JOINER_ONLY: ReadonlySet<string> = new Set(["x"]);
const SIGN_IN_VERBS = ["sign in", "sign up", "log in", "login", "continue", "entrar", "continuar", "acessar", "cadastrar", "cadastre se"];
```

Append to `NEGATIVE`: `"share", "follow", "compartilhar", "seguir"`.

In `providerOf`, replace the inner loop body:

```ts
    for (const name of names) {
      if (!hasPhrase(text, name)) continue;
      const joined = JOINERS.some((j) => hasPhrase(text, `${j} ${name}`));
      if (JOINER_ONLY.has(name) && !(hasAny(text, SIGN_IN_VERBS) && JOINERS.some((j) => text.endsWith(` ${j} ${name}`)))) continue;
      let s = joined ? 80 : text === name || text === `${name} account` ? 40 : 0;
      if (s === 40 && context) s += 20;
      if (linked === p) s += 30;
      if (s > 0 && (!best || s > best.score)) best = { provider: p, score: s };
    }
```

- [ ] **Step 4: Run the extension tests**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/autofill/sso.ts apps/extension/src/autofill/sso.test.ts
git commit -m "feat(extension): recognise Facebook, Discord, X, LinkedIn and GitLab sign-in buttons"
```

---

### Task 3: Rust decides a provider's logins (`sso_accounts`, `provider_login`)

**Files:**
- Modify: `crates/havenkeys-core/src/vault.rs` (near `provider_accounts`, line ~1453; constants near line 47; types near `SaveAction`, line ~188)
- Test: `crates/havenkeys-core/tests/security.rs` (after `provider_accounts_are_the_usernames_the_vault_matches_to_the_provider`)

**Interfaces:**
- Consumes: `VaultService::find_matches(&self, page_url: &str, top_url: Option<&str>) -> Result<Vec<Suggestion>>`, `VaultService::get_item(&self, &Uuid) -> Result<ItemOverview>`, `normalize_username(Option<&str>) -> Option<String>`.
- Produces:
  - `pub const MAX_SSO_PICKER_ACCOUNTS: usize = 50;`
  - `pub struct SsoAccount { pub id: Uuid, pub title: String, pub username: String }` (Serialize, camelCase; `Debug` hides title and username)
  - `pub enum ProviderLogin { One(Uuid), None, Several }` (Serialize as `{"kind":"one","id":"…"}`, `{"kind":"none"}`, `{"kind":"several"}`)
  - `VaultService::sso_accounts(&self, provider: SsoProvider) -> Result<Vec<SsoAccount>>`
  - `VaultService::provider_login(&self, id: &Uuid) -> Result<ProviderLogin>`

- [ ] **Step 1: Write the failing tests**

In `crates/havenkeys-core/tests/security.rs` extend the `use havenkeys_core::vault::{…}` line with `ProviderLogin` and add:

```rust
#[test]
fn sso_accounts_are_the_logins_the_vault_matches_to_the_provider() {
    let (mut v, _) = sso_vault();
    for (rev, input) in (7..).zip([
        login("Google", " Me@Gmail.com ", "pw", "google.com"),
        login("Google (work)", "srocha@callix.com.br", "pw", "https://accounts.google.com"),
        login("Google, no username", "", "pw", "google.com"),
        login("Look-alike", "evil@x.com", "pw", "google.com.evil.com"),
        login("Discord", "samuel", "pw", "discord.com"),
    ]) {
        let staged = v.stage_create(input, NOW).unwrap();
        v.commit_write(staged, rev).unwrap();
    }
    let google = v.sso_accounts(SsoProvider::Google).unwrap();
    let shown: Vec<(&str, &str)> = google.iter().map(|a| (a.title.as_str(), a.username.as_str())).collect();
    // Sorted by title; username trimmed but its case kept; no empty username,
    // no look-alike, no Typeform "Sign in with" login (saved for typeform.com).
    assert_eq!(shown, [("Google", "Me@Gmail.com"), ("Google (work)", "srocha@callix.com.br")]);
    assert_eq!(v.sso_accounts(SsoProvider::Discord).unwrap().len(), 1);
    assert!(v.sso_accounts(SsoProvider::Gitlab).unwrap().is_empty());
    // The save prompt's account list is built on the same rule.
    assert_eq!(
        v.provider_accounts(SsoProvider::Google).unwrap(),
        vec!["me@gmail.com".to_owned(), "srocha@callix.com.br".to_owned()]
    );
    // Never a secret, never the username in Debug output.
    let json = serde_json::to_string(&google[0]).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["id", "title", "username"]);
    assert!(!format!("{:?}", google[0]).contains("Gmail"));
    v.lock();
    assert_eq!(v.sso_accounts(SsoProvider::Google).err(), Some(Error::Locked));
}

#[test]
fn sso_accounts_are_capped() {
    let (mut v, _) = sso_vault();
    for i in 0..(havenkeys_core::vault::MAX_SSO_PICKER_ACCOUNTS + 5) {
        let staged = v
            .stage_create(login(&format!("G{i:03}"), &format!("u{i}@gmail.com"), "pw", "google.com"), NOW)
            .unwrap();
        v.commit_write(staged, 10 + i as i64).unwrap();
    }
    assert_eq!(
        v.sso_accounts(SsoProvider::Google).unwrap().len(),
        havenkeys_core::vault::MAX_SSO_PICKER_ACCOUNTS
    );
}

#[test]
fn provider_login_is_the_one_login_a_run_would_use() {
    let (mut v, typeform) = sso_vault(); // Typeform signs in with Google as me@gmail.com
    assert_eq!(v.provider_login(&typeform).unwrap(), ProviderLogin::None);

    let staged = v.stage_create(login("Google", " ME@gmail.com ", "pw", "google.com"), NOW).unwrap();
    let google = v.commit_write(staged, 7).unwrap().unwrap().id;
    assert_eq!(v.provider_login(&typeform).unwrap(), ProviderLogin::One(google));

    let staged = v.stage_create(login("Google 2", "me@gmail.com", "pw", "https://accounts.google.com"), NOW).unwrap();
    let second = v.commit_write(staged, 8).unwrap().unwrap().id;
    assert_eq!(v.provider_login(&typeform).unwrap(), ProviderLogin::Several);

    // Deleting the extra one makes the link unambiguous again; deleting the
    // last leaves no link.
    let staged = v.stage_delete(&second).unwrap();
    v.commit_write(staged, 9).unwrap();
    assert_eq!(v.provider_login(&typeform).unwrap(), ProviderLogin::One(google));
    let staged = v.stage_delete(&google).unwrap();
    v.commit_write(staged, 10).unwrap();
    assert_eq!(v.provider_login(&typeform).unwrap(), ProviderLogin::None);

    // A login without "Sign in with" has no provider login.
    let plain = plain_login(&mut v);
    assert_eq!(v.provider_login(&plain).unwrap(), ProviderLogin::None);
    assert_eq!(v.provider_login(&Uuid::new_v4()).err(), Some(Error::NotFound));
    assert_eq!(
        serde_json::to_string(&ProviderLogin::One(typeform)).unwrap(),
        format!(r#"{{"kind":"one","id":"{typeform}"}}"#)
    );
    assert_eq!(serde_json::to_string(&ProviderLogin::Several).unwrap(), r#"{"kind":"several"}"#);
    v.lock();
    assert_eq!(v.provider_login(&typeform).err(), Some(Error::Locked));
}

fn plain_login(v: &mut havenkeys_core::vault::VaultService) -> Uuid {
    let staged = v.stage_create(login("Plain", "plain", "pw", "plain.example"), NOW).unwrap();
    v.commit_write(staged, 50).unwrap().unwrap().id
}
```

(`stage_delete(&self, id: &Uuid) -> Result<StagedWrite>` and `commit_write(&mut self, StagedWrite, revision: i64)` are the real signatures; `serde_json` is a normal dependency of havenkeys-core, usable from integration tests.)

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p havenkeys-core --test security sso_accounts provider_login`
Expected: compile error, `no method named sso_accounts`.

- [ ] **Step 3: Implement**

In `vault.rs`, next to `MAX_PROVIDER_ACCOUNTS`:

```rust
/// Most saved logins the editor's "Sign in with" picker lists per provider.
pub const MAX_SSO_PICKER_ACCOUNTS: usize = 50;
```

Next to `SaveAction`:

```rust
/// A login the vault holds for a provider's own sign-in page. No secrets.
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SsoAccount {
    pub id: Uuid,
    pub title: String,
    pub username: String,
}

impl std::fmt::Debug for SsoAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoAccount").field("id", &self.id).finish_non_exhaustive()
    }
}

/// Result of [`VaultService::provider_login`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "lowercase")]
pub enum ProviderLogin {
    One(Uuid),
    None,
    Several,
}
```

Replace `provider_accounts` and add the two methods:

```rust
    /// Every login whose URL rules match one of the provider's own sign-in
    /// origins and that has a username: what a run may fill there. Ordered
    /// by title, then username. No secrets.
    fn provider_logins(&self, provider: SsoProvider) -> Result<Vec<SsoAccount>> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for origin in provider.origins() {
            for s in self.find_matches(&format!("{origin}/"), None)? {
                let Some(username) = s.username.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
                    continue;
                };
                if seen.insert(s.id) {
                    out.push(SsoAccount { id: s.id, title: s.title.clone(), username: username.to_owned() });
                }
            }
        }
        out.sort_by_cached_key(|a| (a.title.to_lowercase(), a.username.to_lowercase(), a.id));
        Ok(out)
    }

    /// The editor's "Sign in with" picker: [`Self::provider_logins`], at
    /// most [`MAX_SSO_PICKER_ACCOUNTS`].
    pub fn sso_accounts(&self, provider: SsoProvider) -> Result<Vec<SsoAccount>> {
        let mut out = self.provider_logins(provider)?;
        out.truncate(MAX_SSO_PICKER_ACCOUNTS);
        Ok(out)
    }

    /// The accounts the vault holds for `provider`: the usernames of
    /// [`Self::provider_logins`], trimmed, lowercased, deduplicated and
    /// sorted, at most [`MAX_PROVIDER_ACCOUNTS`]. The save prompt offers them
    /// instead of a blank field.
    pub fn provider_accounts(&self, provider: SsoProvider) -> Result<Vec<String>> {
        let out: std::collections::BTreeSet<String> = self
            .provider_logins(provider)?
            .iter()
            .filter_map(|a| normalize_username(Some(&a.username)))
            .collect();
        Ok(out.into_iter().take(MAX_PROVIDER_ACCOUNTS).collect())
    }

    /// The provider login a "Sign in with" login's run would fill: the one
    /// login of [`Self::provider_logins`] whose username equals the saved
    /// account (trimmed, case-insensitive), other than the item itself.
    pub fn provider_login(&self, id: &Uuid) -> Result<ProviderLogin> {
        let item = self.get_item(id)?;
        let Some(sso) = item.sign_in_with.as_ref() else {
            return Ok(ProviderLogin::None);
        };
        let Some(want) = normalize_username(sso.account.as_deref()) else {
            return Ok(ProviderLogin::None);
        };
        let hits: Vec<Uuid> = self
            .provider_logins(sso.provider)?
            .into_iter()
            .filter(|a| a.id != *id && normalize_username(Some(&a.username)).as_deref() == Some(want.as_str()))
            .map(|a| a.id)
            .collect();
        Ok(match hits.as_slice() {
            [one] => ProviderLogin::One(*one),
            [] => ProviderLogin::None,
            _ => ProviderLogin::Several,
        })
    }
```

- [ ] **Step 4: Run the core tests and clippy**

Run: `cargo test -p havenkeys-core && cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: all pass, including the unchanged `provider_accounts_are_the_usernames_the_vault_matches_to_the_provider`.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core/src/vault.rs crates/havenkeys-core/tests/security.rs
git commit -m "feat(core): sso_accounts and provider_login decide a provider's logins in Rust"
```

---

### Task 4: Tauri commands and the desktop API

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands.rs` (after `get_item`), `apps/desktop/src-tauri/src/lib.rs:284-286` (handler list), `apps/desktop/src-tauri/build.rs` (`COMMANDS`), `apps/desktop/src-tauri/capabilities/main.json`
- Modify: `apps/desktop/src/lib/types.ts`, `apps/desktop/src/lib/api.ts:92-94`
- Test: `apps/desktop/src/lib/commands.test.ts` (existing; checks the four lists agree)

**Interfaces:**
- Consumes: `VaultService::sso_accounts`, `VaultService::provider_login`, `SsoAccount`, `ProviderLogin` (Task 3).
- Produces (TS):
  - `export interface SsoAccount { id: string; title: string; username: string }`
  - `export type ProviderLogin = { kind: "one"; id: string } | { kind: "none" } | { kind: "several" };`
  - `api.ssoAccounts(provider: SsoProvider): Promise<SsoAccount[]>`
  - `api.providerLogin(id: string): Promise<ProviderLogin>`

- [ ] **Step 1: Add the API wrappers first (the agreement test then fails)**

`apps/desktop/src/lib/types.ts`, after `SignInWith`:

```ts
/** A saved login for a provider's own sign-in page (Rust `SsoAccount`). No secrets. */
export interface SsoAccount {
  id: string;
  title: string;
  username: string;
}

/** Which login a "Sign in with" login's provider login is (Rust `ProviderLogin`). */
export type ProviderLogin = { kind: "one"; id: string } | { kind: "none" } | { kind: "several" };
```

`apps/desktop/src/lib/api.ts`, after `getItem` (add `ProviderLogin`, `SsoAccount`, `SsoProvider` to the type import):

```ts
  /** The vault's logins for a provider's sign-in page, for the editor's picker. */
  ssoAccounts: (provider: SsoProvider) => call<SsoAccount[]>("sso_accounts", { provider }),
  /** The provider login a "Sign in with" login links to, decided in Rust. */
  providerLogin: (id: string) => call<ProviderLogin>("provider_login", { id }),
```

- [ ] **Step 2: Run the agreement test to see it fail**

Run: `pnpm --filter @havenkeys/desktop exec vitest run src/lib/commands.test.ts`
Expected: FAIL naming `sso_accounts` and `provider_login` as missing from build.rs / lib.rs / main.json.

- [ ] **Step 3: Rust commands and registration**

`commands.rs`, extend imports: `use havenkeys_core::sso::SsoProvider;` and `use havenkeys_core::vault::{ProviderLogin, SsoAccount, StagedWrite, VaultService, VaultStatus};`. After `get_item`:

```rust
#[tauri::command]
pub fn sso_accounts(
    state: State<'_, AppState>,
    provider: SsoProvider,
) -> CmdResult<Vec<SsoAccount>> {
    // Read-only, no secrets; no `touch()`, like `get_item`.
    Ok(state.vault()?.sso_accounts(provider)?)
}

#[tauri::command]
pub fn provider_login(state: State<'_, AppState>, id: Uuid) -> CmdResult<ProviderLogin> {
    Ok(state.vault()?.provider_login(&id)?)
}
```

`lib.rs` handler list after `commands::get_item,`:

```rust
            commands::sso_accounts,
            commands::provider_login,
```

`build.rs` `COMMANDS` after `"get_item",`: `"sso_accounts",` and `"provider_login",`.

`capabilities/main.json` after `"allow-get-item",`: `"allow-sso-accounts",` and `"allow-provider-login",`.

- [ ] **Step 4: Run checks**

Run: `pnpm --filter @havenkeys/desktop exec vitest run src/lib/commands.test.ts && pnpm --filter @havenkeys/desktop typecheck && cargo check -p havenkeys-desktop`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/build.rs apps/desktop/src-tauri/capabilities/main.json apps/desktop/src/lib/types.ts apps/desktop/src/lib/api.ts
git commit -m "feat(desktop): sso_accounts and provider_login commands"
```

---

### Task 5: Detail view asks Rust for the provider login

**Files:**
- Create: `apps/desktop/src/components/SsoRow.tsx`, `apps/desktop/src/components/SsoRow.test.tsx`
- Modify: `apps/desktop/src/views/ItemDetail.tsx:6,18,169,218-232`, `apps/desktop/src/views/VaultScreen.tsx` (drop `items` prop on `ItemDetail`)
- Modify: `apps/desktop/src/lib/sso.ts` (delete `providerLogin`, `PROVIDER_DOMAINS`, `hostOf`), `apps/desktop/src/lib/sso.test.ts` (delete the `providerLogin` describe block)
- Modify: `apps/desktop/src/i18n/en.ts` (`detail`), `apps/desktop/src/i18n/pt-BR.ts` (`detail`)

**Interfaces:**
- Consumes: `api.providerLogin(id: string): Promise<ProviderLogin>` (Task 4); `PROVIDER_NAMES`; `Field`, `IconButton` from `components/Field.tsx`; `ProviderIcon`.
- Produces: `SsoRow({ item, onOpen }: { item: ItemOverview; onOpen: (id: string) => void })`; i18n `detail.noProviderLogin(provider: string)`, `detail.severalProviderLogins(provider: string)`.

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src/components/SsoRow.test.tsx`:

```tsx
// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ItemOverview, ProviderLogin } from "../lib/types";

const providerLogin = vi.fn<(id: string) => Promise<ProviderLogin>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: { providerLogin: (id: string) => providerLogin(id) },
}));

import { en } from "../i18n/en";
import { SsoRow } from "./SsoRow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const vercel: ItemOverview = {
  id: "vercel", itemType: "login", title: "Vercel", username: null, urls: [], hasPassword: false, hasTotp: false,
  hasNotes: false, hasPasskey: false, autoSignIn: true, createdAt: 0, updatedAt: 0,
  signInWith: { provider: "google", account: "me@gmail.com" },
};

let host: HTMLElement;
let root: Root;
beforeEach(() => {
  providerLogin.mockReset();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("SsoRow", () => {
  it("opens the one provider login", async () => {
    providerLogin.mockResolvedValue({ kind: "one", id: "google" });
    const onOpen = vi.fn();
    await act(async () => root.render(<SsoRow item={vercel} onOpen={onOpen} />));
    expect(providerLogin).toHaveBeenCalledWith("vercel");
    const open = host.querySelector<HTMLButtonElement>(`button[aria-label="${en.detail.openProviderLogin("Google")}"]`)!;
    await act(async () => open.click());
    expect(onOpen).toHaveBeenCalledWith("google");
    expect(host.textContent).not.toContain(en.detail.noProviderLogin("Google"));
  });
  it("says when no saved login matches", async () => {
    providerLogin.mockResolvedValue({ kind: "none" });
    await act(async () => root.render(<SsoRow item={vercel} onOpen={() => {}} />));
    expect(host.textContent).toContain(en.detail.noProviderLogin("Google"));
    expect(host.querySelector("button")).toBeNull();
  });
  it("says when several saved logins match", async () => {
    providerLogin.mockResolvedValue({ kind: "several" });
    await act(async () => root.render(<SsoRow item={vercel} onOpen={() => {}} />));
    expect(host.textContent).toContain(en.detail.severalProviderLogins("Google"));
  });
  it("shows no note for a provider without an account, or on an error", async () => {
    providerLogin.mockResolvedValue({ kind: "none" });
    const bare = { ...vercel, signInWith: { provider: "github" as const, account: null } };
    await act(async () => root.render(<SsoRow item={bare} onOpen={() => {}} />));
    expect(host.textContent).toContain("GitHub");
    expect(host.textContent).not.toContain(en.detail.noProviderLogin("GitHub"));
    providerLogin.mockRejectedValue(new Error("locked"));
    await act(async () => root.render(<SsoRow item={{ ...vercel, id: "other" }} onOpen={() => {}} />));
    expect(host.textContent).not.toContain(en.detail.noProviderLogin("Google"));
  });
});
```

- [ ] **Step 2: Run it to see it fail**

Run: `pnpm --filter @havenkeys/desktop exec vitest run src/components/SsoRow.test.tsx`
Expected: FAIL, cannot resolve `./SsoRow`.

- [ ] **Step 3: Strings**

`en.ts`, in `detail` after `openProviderLogin`:

```ts
    noProviderLogin: (provider: string) => `No saved ${provider} login for this account`,
    severalProviderLogins: (provider: string) => `Several saved ${provider} logins match this account`,
```

`pt-BR.ts`, same place:

```ts
    noProviderLogin: (provider: string) => `Nenhum login do ${provider} salvo para esta conta`,
    severalProviderLogins: (provider: string) => `Vários logins do ${provider} salvos correspondem a esta conta`,
```

- [ ] **Step 4: Implement `SsoRow`**

Create `apps/desktop/src/components/SsoRow.tsx`:

```tsx
// The "Sign in with" row of a login's detail view. Which login it links to
// is decided in Rust (`provider_login`), the same rule a sign-in run uses.

import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { PROVIDER_NAMES } from "../lib/sso";
import type { ItemOverview, ProviderLogin } from "../lib/types";
import { useI18n } from "../i18n/context";
import { Field, IconButton } from "./Field";
import { ProviderIcon } from "./ProviderIcon";

export function SsoRow({ item, onOpen }: { item: ItemOverview; onOpen: (id: string) => void }) {
  const { t } = useI18n();
  const [link, setLink] = useState<ProviderLogin | null>(null);
  const sso = item.signInWith;

  useEffect(() => {
    let live = true;
    setLink(null);
    api.providerLogin(item.id).then(
      (l) => live && setLink(l),
      () => live && setLink(null),
    );
    return () => {
      live = false;
    };
  }, [item.id, item.updatedAt]);

  if (!sso) return null;
  const name = PROVIDER_NAMES[sso.provider];
  const note =
    sso.account && link?.kind === "none"
      ? t.detail.noProviderLogin(name)
      : sso.account && link?.kind === "several"
        ? t.detail.severalProviderLogins(name)
        : null;
  return (
    <Field
      label={t.detail.signInWith}
      actions={link?.kind === "one" ? <IconButton icon="arrowRight" label={t.detail.openProviderLogin(name)} onClick={() => onOpen(link.id)} /> : undefined}
    >
      <span className="sso-value">
        <ProviderIcon provider={sso.provider} />
        <span>{name}</span>
        {sso.account && <span className="muted selectable" data-truncate="">{sso.account}</span>}
      </span>
      {note && <span className="sso-note muted">{note}</span>}
    </Field>
  );
}
```

`styles.css`, near `.sso-value`:

```css
.sso-note {
  display: block;
  margin-top: 3px;
  font-size: 12px;
}
```

- [ ] **Step 5: Use it in the detail view; delete the TS matcher**

In `ItemDetail.tsx` replace the IIFE block at lines 218-232 with:

```tsx
            {item.signInWith && <SsoRow item={item} onOpen={onOpen} />}
```

Remove the `items` prop from `Props` and the function signature, import `SsoRow`, and drop `providerLogin` from the `../lib/sso` import (keep `PROVIDER_NAMES` only if still used elsewhere in the file). In `VaultScreen.tsx` remove `items={allItems}` from `<ItemDetail …>` (keep `allItems` itself: `selected` and `identity` use it).

In `lib/sso.ts` delete `PROVIDER_DOMAINS`, `hostOf` and `providerLogin` (and the now-unused `ItemOverview` import). In `lib/sso.test.ts` delete the whole `describe("providerLogin", …)` block except the `formats the subtitle` test, which moves into a `describe("ssoSubtitle", …)` block; remove `providerLogin` and `ItemOverview`/`item` helpers if unused.

- [ ] **Step 6: Run the desktop tests and typecheck**

Run: `pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck`
Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/components/SsoRow.tsx apps/desktop/src/components/SsoRow.test.tsx apps/desktop/src/views/ItemDetail.tsx apps/desktop/src/views/VaultScreen.tsx apps/desktop/src/lib/sso.ts apps/desktop/src/lib/sso.test.ts apps/desktop/src/i18n/en.ts apps/desktop/src/i18n/pt-BR.ts apps/desktop/src/styles.css
git commit -m "feat(desktop): the Sign in with row links to the provider login Rust picks, and says when none or several match"
```

---

### Task 6: The editor's picker

**Files:**
- Modify: `apps/desktop/src/lib/sso.ts` (row model), `apps/desktop/src/lib/sso.test.ts`
- Create: `apps/desktop/src/components/SsoPicker.tsx`, `apps/desktop/src/components/SsoPicker.test.tsx`
- Modify: `apps/desktop/src/views/ItemEditor.tsx:213-249`, `apps/desktop/src/i18n/en.ts` / `pt-BR.ts` (`editor`), `apps/desktop/src/styles.css`

**Interfaces:**
- Consumes: `api.ssoAccounts(provider): Promise<SsoAccount[]>` (Task 4); `PROVIDER_ORDER`, `PROVIDER_NAMES` (Task 1); `ProviderIcon`; `Icon`.
- Produces:
  - `export type PickerRow = { kind: "provider"; provider: SsoProvider } | { kind: "account"; provider: SsoProvider; account: SsoAccount } | { kind: "none" };`
  - `export function pickerRows(accounts: Partial<Record<SsoProvider, SsoAccount[]>>, query: string): PickerRow[]`
  - `export function applyRow(row: PickerRow, current: SignInWith | null): SignInWith | null`
  - `SsoPicker({ value, onChange }: { value: SignInWith | null; onChange: (v: SignInWith | null) => void })`
  - i18n `editor.providerSearch`, `editor.noMatches`

- [ ] **Step 1: Write the failing row-model tests**

Append to `apps/desktop/src/lib/sso.test.ts` (import `pickerRows`, `applyRow` from `./sso` and `SsoAccount` from `./types`):

```ts
describe("pickerRows", () => {
  const g1: SsoAccount = { id: "g1", title: "google.com", username: "samuelsilv.rocha@gmail.com" };
  const gh: SsoAccount = { id: "gh", title: "Github", username: "rochasamuel" };
  const accounts = { google: [g1], github: [gh] };
  const label = (r: PickerRow) => (r.kind === "none" ? "none" : r.kind === "provider" ? r.provider : `${r.provider}:${r.account.id}`);

  it("lists every provider in order, its saved logins under it, then None", () => {
    expect(pickerRows(accounts, "").map(label)).toEqual([
      "apple", "discord", "facebook", "github", "github:gh", "gitlab", "google", "google:g1", "linkedin", "microsoft", "x", "none",
    ]);
  });
  it("filters by provider name, title and username, case-insensitively", () => {
    expect(pickerRows(accounts, "GOO").map(label)).toEqual(["google", "google:g1"]);
    expect(pickerRows(accounts, "rochasam").map(label)).toEqual(["github", "github:gh"]);
    expect(pickerRows(accounts, "rocha").map(label)).toEqual(["github", "github:gh", "google", "google:g1"]);
    expect(pickerRows(accounts, "zzz")).toEqual([]);
  });
});

describe("applyRow", () => {
  const gh: SsoAccount = { id: "gh", title: "Github", username: "rochasamuel" };
  it("copies a saved login's username", () => {
    expect(applyRow({ kind: "account", provider: "github", account: gh }, null)).toEqual({ provider: "github", account: "rochasamuel" });
  });
  it("keeps the account when the provider stays, clears it when it changes", () => {
    const cur = { provider: "google" as const, account: "me@gmail.com" };
    expect(applyRow({ kind: "provider", provider: "google" }, cur)).toEqual(cur);
    expect(applyRow({ kind: "provider", provider: "x" }, cur)).toEqual({ provider: "x", account: null });
  });
  it("None clears", () => {
    expect(applyRow({ kind: "none" }, { provider: "google", account: "a" })).toBeNull();
  });
});
```

- [ ] **Step 2: Run them to see them fail**

Run: `pnpm --filter @havenkeys/desktop exec vitest run src/lib/sso.test.ts`
Expected: FAIL, `pickerRows` is not exported.

- [ ] **Step 3: Implement the row model**

Append to `apps/desktop/src/lib/sso.ts` (add `SignInWith`, `SsoAccount` to the type import):

```ts
/** One row of the editor's "Sign in with" picker. */
export type PickerRow =
  | { kind: "provider"; provider: SsoProvider }
  | { kind: "account"; provider: SsoProvider; account: SsoAccount }
  | { kind: "none" };

/**
 * The picker's rows: each provider (in PROVIDER_ORDER) followed by the
 * vault's logins for it, then "None". A query keeps a provider whose name
 * matches (with all its logins) or whose logins match by title or username
 * (with just those); "None" only shows with no query.
 */
export function pickerRows(accounts: Partial<Record<SsoProvider, SsoAccount[]>>, query: string): PickerRow[] {
  const q = query.trim().toLowerCase();
  const rows: PickerRow[] = [];
  for (const provider of PROVIDER_ORDER) {
    const all = accounts[provider] ?? [];
    const nameHit = q === "" || PROVIDER_NAMES[provider].toLowerCase().includes(q);
    const hits = nameHit ? all : all.filter((a) => a.title.toLowerCase().includes(q) || a.username.toLowerCase().includes(q));
    if (!nameHit && hits.length === 0) continue;
    rows.push({ kind: "provider", provider });
    for (const account of hits) rows.push({ kind: "account", provider, account });
  }
  if (q === "") rows.push({ kind: "none" });
  return rows;
}

/** What picking `row` sets "Sign in with" to. */
export function applyRow(row: PickerRow, current: SignInWith | null): SignInWith | null {
  switch (row.kind) {
    case "none":
      return null;
    case "account":
      return { provider: row.provider, account: row.account.username };
    case "provider":
      return { provider: row.provider, account: current?.provider === row.provider ? current.account : null };
  }
}
```

Run: `pnpm --filter @havenkeys/desktop exec vitest run src/lib/sso.test.ts` → PASS.

- [ ] **Step 4: Write the failing component tests**

Create `apps/desktop/src/components/SsoPicker.test.tsx`:

```tsx
// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SignInWith, SsoAccount, SsoProvider } from "../lib/types";

const ssoAccounts = vi.fn<(p: SsoProvider) => Promise<SsoAccount[]>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: { ssoAccounts: (p: SsoProvider) => ssoAccounts(p) },
}));

import { en } from "../i18n/en";
import { SsoPicker } from "./SsoPicker";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const g1: SsoAccount = { id: "g1", title: "google.com", username: "samuelsilv.rocha@gmail.com" };
let host: HTMLElement;
let root: Root;
let value: SignInWith | null;
const onChange = vi.fn((v: SignInWith | null) => {
  value = v;
});

beforeEach(() => {
  value = null;
  onChange.mockClear();
  ssoAccounts.mockReset().mockImplementation(async (p) => (p === "google" ? [g1] : []));
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

const trigger = () => host.querySelector<HTMLButtonElement>("button.sso-trigger")!;
const options = () => [...host.querySelectorAll<HTMLElement>('[role="option"]')];
const search = () => host.querySelector<HTMLInputElement>('input[type="search"]')!;
function type(v: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
  setter.call(search(), v);
  search().dispatchEvent(new Event("input", { bubbles: true }));
}
const key = (k: string) => search().dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true }));

describe("SsoPicker", () => {
  it("lists providers with their saved logins and copies the picked login's username", async () => {
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    expect(trigger().textContent).toContain(en.editor.providerNone);
    await act(async () => trigger().click());
    expect(ssoAccounts).toHaveBeenCalledTimes(9);
    const row = options().find((o) => o.textContent?.includes("samuelsilv.rocha@gmail.com"))!;
    await act(async () => row.click());
    expect(onChange).toHaveBeenLastCalledWith({ provider: "google", account: "samuelsilv.rocha@gmail.com" });
    expect(host.querySelector('[role="listbox"]')).toBeNull();
  });
  it("shows the stored value of an existing login and loads accounts only once", async () => {
    value = { provider: "github", account: "rochasamuel" };
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    expect(trigger().textContent).toContain("GitHub");
    expect(trigger().textContent).toContain("rochasamuel");
    await act(async () => trigger().click());
    await act(async () => key("Escape"));
    await act(async () => trigger().click());
    expect(ssoAccounts).toHaveBeenCalledTimes(9);
    expect(onChange).not.toHaveBeenCalled();
  });
  it("filters, moves with arrows, picks with Enter", async () => {
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    await act(async () => type("goo"));
    expect(options().length).toBe(2);
    await act(async () => key("ArrowDown"));
    await act(async () => key("Enter"));
    expect(onChange).toHaveBeenLastCalledWith({ provider: "google", account: "samuelsilv.rocha@gmail.com" });
  });
  it("says No matches; Enter does nothing; Escape keeps the value", async () => {
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    await act(async () => type("zzz"));
    expect(options().length).toBe(0);
    expect(host.textContent).toContain(en.editor.noMatches);
    await act(async () => key("Enter"));
    await act(async () => key("Escape"));
    expect(onChange).not.toHaveBeenCalled();
    expect(host.querySelector('[role="listbox"]')).toBeNull();
  });
  it("still lists the providers when the vault is locked", async () => {
    ssoAccounts.mockRejectedValue(new Error("locked"));
    await act(async () => root.render(<SsoPicker value={value} onChange={onChange} />));
    await act(async () => trigger().click());
    expect(options().map((o) => o.textContent)).toEqual(
      expect.arrayContaining(["Apple", "Discord", "Facebook", "GitHub", "GitLab", "Google", "LinkedIn", "Microsoft", "X"]),
    );
  });
});
```

Run: `pnpm --filter @havenkeys/desktop exec vitest run src/components/SsoPicker.test.tsx`
Expected: FAIL, cannot resolve `./SsoPicker`.

- [ ] **Step 5: Strings**

`en.ts` `editor`, after `providerPick`:

```ts
    providerSearch: "Search providers and logins",
    noMatches: "No matches",
```

`pt-BR.ts` `editor`:

```ts
    providerSearch: "Buscar provedores e logins",
    noMatches: "Nada encontrado",
```

- [ ] **Step 6: Implement `SsoPicker`**

Create `apps/desktop/src/components/SsoPicker.tsx`:

```tsx
// "Sign in with" picker: each provider and, under it, the vault's own logins
// for that provider (from Rust's `sso_accounts`, no secrets). Picking a login
// copies its username as the account; the editor's account field stays
// editable for an account the vault does not hold.

import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import { applyRow, pickerRows, PROVIDER_NAMES, PROVIDER_ORDER, type PickerRow } from "../lib/sso";
import type { SignInWith, SsoAccount, SsoProvider } from "../lib/types";
import { useI18n } from "../i18n/context";
import { Icon } from "./Icon";
import { ProviderIcon } from "./ProviderIcon";

type Accounts = Partial<Record<SsoProvider, SsoAccount[]>>;

export function SsoPicker({ value, onChange }: { value: SignInWith | null; onChange: (v: SignInWith | null) => void }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [accounts, setAccounts] = useState<Accounts | null>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const rows = useMemo(() => pickerRows(accounts ?? {}, query), [accounts, query]);

  // Loaded once per editor, on first open.
  useEffect(() => {
    if (!open || accounts) return;
    let live = true;
    void Promise.allSettled(PROVIDER_ORDER.map((p) => api.ssoAccounts(p))).then((results) => {
      if (!live) return;
      const out: Accounts = {};
      results.forEach((r, i) => {
        if (r.status === "fulfilled") out[PROVIDER_ORDER[i] as SsoProvider] = r.value;
      });
      setAccounts(out);
    });
    return () => {
      live = false;
    };
  }, [open, accounts]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (wrap.current && !wrap.current.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  function close() {
    setOpen(false);
    setQuery("");
    setActive(0);
  }

  function pick(row: PickerRow) {
    onChange(applyRow(row, value));
    close();
  }

  function onKey(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, rows.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const row = rows[active];
      if (row) pick(row);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  const rowId = (i: number) => `sso-option-${i}`;
  return (
    <div className="sso-picker" ref={wrap}>
      <button
        type="button"
        className="sso-trigger edit-input"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={t.editor.providerPick}
        onClick={() => (open ? close() : setOpen(true))}
      >
        {value ? (
          <>
            <ProviderIcon provider={value.provider} />
            <span>{PROVIDER_NAMES[value.provider]}</span>
            {value.account && <span className="muted" data-truncate="">· {value.account}</span>}
          </>
        ) : (
          <span className="muted">{t.editor.providerNone}</span>
        )}
        <Icon name="chevronDown" size={14} className="select-chevron" />
      </button>
      {open && (
        <div className="menu sso-menu">
          <input
            type="search"
            className="edit-input sso-search"
            value={query}
            placeholder={t.editor.providerSearch}
            aria-label={t.editor.providerSearch}
            aria-controls="sso-listbox"
            aria-activedescendant={rows[active] ? rowId(active) : undefined}
            autoFocus
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            onKeyDown={onKey}
          />
          <div role="listbox" id="sso-listbox" aria-label={t.editor.providerPick}>
            {rows.map((row, i) => (
              <div
                key={row.kind === "account" ? `a-${row.account.id}` : row.kind === "provider" ? `p-${row.provider}` : "none"}
                id={rowId(i)}
                role="option"
                aria-selected={i === active}
                className={`sso-option${row.kind === "account" ? " sso-option-account" : ""}`}
                onMouseEnter={() => setActive(i)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => pick(row)}
              >
                {row.kind === "none" ? (
                  <span>{t.editor.providerNone}</span>
                ) : row.kind === "provider" ? (
                  <>
                    <ProviderIcon provider={row.provider} />
                    <span>{PROVIDER_NAMES[row.provider]}</span>
                  </>
                ) : (
                  <>
                    <ProviderIcon provider={row.provider} size={15} />
                    <span data-truncate="">{row.account.title}</span>
                    <span className="muted" data-truncate="">{row.account.username}</span>
                  </>
                )}
              </div>
            ))}
            {rows.length === 0 && <p className="sso-empty muted">{t.editor.noMatches}</p>}
          </div>
        </div>
      )}
    </div>
  );
}
```

Notes for the implementer: the "still lists the providers when locked" test's provider-row `textContent` is just the name (the icon is an `<svg>` with no text), which is why it compares names exactly. The `keydown` events in the tests are dispatched on the search input; React's `onKeyDown` receives them. `Icon` accepts `className`.

`styles.css`, after `.menu-heading`:

```css
.sso-picker {
  position: relative;
  flex: 1;
  min-width: 0;
}

.sso-trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  text-align: left;
  cursor: pointer;
}

.sso-trigger .select-chevron {
  margin-left: auto;
}

.sso-menu {
  left: 0;
  right: auto;
  width: min(360px, 100%);
  max-height: 340px;
  overflow-y: auto;
  transform-origin: top left;
}

.sso-search {
  width: 100%;
  margin-bottom: 4px;
}

.sso-option {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 7px 9px;
  border-radius: 7px;
  color: var(--text-strong);
  cursor: pointer;
}

.sso-option[aria-selected="true"] {
  background: var(--sel);
}

.sso-option-account {
  padding-left: 30px;
  font-size: 13px;
}

.sso-option-account .muted {
  margin-left: auto;
}

.sso-empty {
  padding: 8px 9px;
  font-size: 12.5px;
}
```

- [ ] **Step 7: Put it in the editor**

In `ItemEditor.tsx` replace the `<span className="select-wrap">…</span>` block of the "Sign in with" row (lines ~217-233) with:

```tsx
              <SsoPicker
                value={signIn}
                onChange={(next) => {
                  setSignIn(next);
                  if (!next) setPasswordOpen(true);
                  else if (shouldCollapse(username, password, existing?.hasPassword ?? false)) setPasswordOpen(false);
                }}
              />
```

Import `SsoPicker` from `../components/SsoPicker`; remove `PROVIDER_NAMES`, `PROVIDER_ORDER` and `SsoProvider` from the imports if no longer used. The account `<input>` below stays as is.

- [ ] **Step 8: Run the desktop suite and typecheck**

Run: `pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck`
Expected: pass.

- [ ] **Step 9: Look at it**

Run the desktop app (`pnpm dev`, or the `run` skill), open a login, Edit, open "Sign in with": providers alphabetical, saved Google/GitHub logins indented under them, filter, keyboard, pick → account filled, Save → detail shows the arrow to the provider login. Check light and dark themes and a narrow window. Fix layout issues before committing.

- [ ] **Step 10: Commit**

```bash
git add apps/desktop/src/lib/sso.ts apps/desktop/src/lib/sso.test.ts apps/desktop/src/components/SsoPicker.tsx apps/desktop/src/components/SsoPicker.test.tsx apps/desktop/src/views/ItemEditor.tsx apps/desktop/src/i18n/en.ts apps/desktop/src/i18n/pt-BR.ts apps/desktop/src/styles.css
git commit -m "feat(desktop): pick the Sign in with account from the vault's provider logins"
```

---

### Task 7: Docs and full verification

**Files:**
- Modify: `docs/autofill.md` (section "Sign in with Google, Microsoft, GitHub, Apple", line ~558), `docs/security-model.md` (§17 heading at line ~817, Tauri command table at line ~212), `docs/native-messaging.md` (provider list, if it enumerates providers: `grep -n "apple" docs/native-messaging.md`)

- [ ] **Step 1: Update the docs**

- `docs/autofill.md`: rename the section to "Sign in with a provider (Google, Microsoft, GitHub, Apple, Facebook, Discord, X, LinkedIn, GitLab)"; add the provider → origins table from the spec §3 (with the origins confirmed in Task 1 Step 1); add one paragraph: "X is recognised only as '<sign-in verb> … with/com X' at the end of a button label, never as a bare 'X' (a close button); share and follow buttons are ignored."
- `docs/security-model.md` §17: same heading change, and a bullet: "**Which login is a provider's is decided in Rust.** `sso_accounts` (editor picker) and `provider_login` (detail link) use the same rule as a run and as `check_sso`: logins matched by `find_matches` to the provider's fixed origins, whose username equals the saved account. Both return ids, titles and usernames only." Plus a compatibility bullet: "An app older than this change cannot decode a login whose provider is Facebook, Discord, X, LinkedIn or GitLab and reports it as unreadable until updated; desktop, extension and Android are released together."
- Command table: add rows
  `| sso_accounts | yes | no (id, title, username of the provider's logins, at most 50) |`
  `| provider_login | yes | no (one id, or none/several) |`
- Update the `native-messaging.md` provider enum list if present.

- [ ] **Step 2: Full verification**

Run, in order, and read every result:

```bash
cargo fmt --check
pnpm lint:rust
cargo test
pnpm -r typecheck
pnpm -r test
pnpm build:extension
```

Expected: all pass, no warnings from clippy. Then grep for leftovers: `grep -rn "PROVIDER_DOMAINS\|providerLogin(items" apps packages` → no results.

- [ ] **Step 3: Commit**

```bash
git add docs/autofill.md docs/security-model.md docs/native-messaging.md
git commit -m "docs: nine Sign in with providers; provider logins decided in Rust"
```

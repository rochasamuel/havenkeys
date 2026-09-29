# Sign in with … : complete the provider's login — Design

Status: proposed, 2026-09-29.
Amends `2026-09-28-sign-in-with-design.md` (§2 "Provider page" decision,
§6.2 step 4, §6.3, §7) and the CLAUDE.md §25 note of the same date.

> This software has not undergone an independent security audit.

## 1. Goal

Today a sign-in-with run clicks the saved account on the provider's chooser
and stops there. If the account is not signed in at the provider (no row, or
the provider asks for the password), the user has to finish by hand.

1Password finishes it. HavenKeys should too. After the user picks a
"Sign in with Google · me@gmail.com" login, it continues on the provider's
page:

1. Click "Use another account" when the saved account is not listed.
2. Fill the provider login's email and press Next.
3. Fill its password and press Sign in.
4. Fill its TOTP code if the provider asks and the login has one.

It still stops rather than guess.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Which vault login fills the provider page | **Derived by account**: the login Rust matches to the provider page (`find_matches`) whose username equals the saved account (trimmed, case-insensitive). It must be exactly one, otherwise stop | Explicit item link stored on the site login (breaks on delete, needs an editor control) |
| Which switches must be on | All three: the vault's automatic sign-in, the site login's switch, and the provider login's own switch | Only vault + site switch; a new per-login switch |
| How the provider page is filled | Hand off to the **existing automatic sign-in run** (username → password → OTP) for the provider login | A provider-specific filler |
| Scope of the cross-origin step | Once, within the sign-in-with run's 2 minutes, only on origins Rust returned with `start_sso`, only in the top frame of the run's tab or of a popup that tab opened | Any frame; any origin of the provider's company |

## 3. Behaviour on the provider page

These steps apply only in a **top frame**, on an origin in the run's
`providerOrigins`, in the run's tab or a popup whose `openerTabId` is that
tab, while a sign-in-with run is live. Everywhere else nothing changes.

1. **Chooser row present** for the saved account (unchanged, §6.2 of the
   base spec): click it. If the provider then shows its password step
   (a session that expired), continue at step 3.
2. **No row for the account, but a "Use another account" control**
   ("Use another account", "Usar outra conta", "Sign in with a different
   account", "Usar uma conta diferente"): click it once. Recognised like the
   other controls: fixed phrases, a unique clear winner, visible and enabled,
   never a consent control.
3. **A login form appears** (an email/username field or a password field;
   the existing `findLoginGroup`): the content script asks the background to
   complete the login (`cs_sso_login`). The background then:
   1. accepts it only for a live run in phase `login` (§4), from a top frame
      of the run's tab or its opener popup, on a `providerOrigins` origin;
   2. calls `find_matches` for that frame and keeps logins whose username
      equals the run's account. If there is not exactly one, it ends the run
      and the normal field menu stays available;
   3. calls `fill_item` for that login and URL. Rust re-checks the login
      against the provider page, as for every fill;
   4. only if the run's `autoChoose` is on **and** `fill_item.autoSubmit` is
      on, hands the fill to `pickFill` with `auto` set. That starts an
      ordinary automatic sign-in run bound to the provider origin: username →
      Next → password → Sign in → OTP.
      - If the provider login's switch is off (`autoSubmit` false), it fills
        what fits on the current step (usually the email) and presses
        nothing.
   5. ends the sign-in-with run. From here the automatic sign-in run's own
      limits apply (2 minutes, one origin, stops on user input or a visible
      CAPTCHA, never retries).

## 4. Run phases

`SsoRun.phase` becomes `press → choose → login → end`:

* `pressed()` moves `press → choose`, as before. It needs `autoChoose` and
  an account.
* `chooseFor()`, called on the provider top frame's `cs_ready`, returns the
  account and moves `choose → login`. It no longer ends the run.
* `cs_ready` of a later provider top-frame document while in `login` returns
  `{kind: "login"}`. This is the password page after a chooser click or after
  "Use another account".
* `loginFor()` (the `cs_sso_login` check) consumes the run: `login → end`.
* Any other origin, user input, lock or the 2-minute TTL ends it, as today.

`SsoReady` gains `{kind: "login"}`. The content script watches for a login
form for up to `CHOOSE_WAIT_MS` (10 s), using the same bounded observer as
the chooser, then sends `cs_sso_login` once.

## 5. Security

* The new part: the user's pick on site X authorizes, **once**, filling the
  provider login's username, password and TOTP on the provider's own origin
  without a second click there. It is bounded by:
  * origins from Rust;
  * top frame of the run's tab or its opener popup;
  * 2 minutes;
  * all three switches;
  * exactly one matching provider login;
  * Rust's origin check on `fill_item`.
* The extension gains no secret it could not already request: `fill_item`
  for a login saved for `accounts.google.com`, asked from
  `accounts.google.com`, was always allowed. What changes is that no click on
  that page precedes it.
* Nothing is sent to site X. A malicious site X can at most start a run that
  ends on the real provider page, with the user's own provider login, after
  the user picked its sign-in-with row. It cannot see the provider page.
* Unchanged:
  * consent screens are never pressed;
  * a visible CAPTCHA stops the run;
  * embedded provider iframes (GSI, One Tap) never act.
* CLAUDE.md §25 gains an amendment note pointing here.

## 6. Code (sketch)

* `autofill/sso.ts`: `anotherAccountButton(root, env)`, with the phrases of
  §3 step 2, a unique clear winner, and no consent labels.
* `messaging/sso.ts`:
  * `SsoReady` gains `{kind: "login"}`;
  * new content request `cs_sso_login` with no fields: the frame comes from
    sender data.
* `background/sso-state.ts`: the `login` phase, `chooseFor` moves to
  `login`, new `loginFor(tab, origin): SsoRun | null`, and `ready` returns
  `login` for a later provider top-frame document.
* `background/sso-handler.ts`: `cs_sso_login` → §3 step 3 using
  `find_matches`, `fill_item` and the inline handler's `pickFill` (a new dep).
* `content/sso.ts`: the choose watcher gains the "Use another account" click
  and the login-form hand-off. `onReady({kind:"login"})` watches for the
  login form.
* No Rust change.

## 7. Testing

* **sso-state:**
  * phase transitions;
  * `loginFor` only on provider top frames of the run's tab or opener popup;
  * only once;
  * not after TTL, lock or `topLoad` elsewhere.
* **sso-handler:**
  * exactly one provider login with that username → `fill_item` + `pickFill`
    with auto;
  * zero or two → run ends, no fill;
  * a different username is never filled;
  * `autoSubmit` false → fill without press;
  * a subframe `cs_sso_login` → nothing;
  * a denied `fill_item` → run ends.
* **autofill/sso:**
  * "Use another account" in English and pt-BR;
  * no match on consent screens;
  * ambiguity → null.
* **content:**
  * no row + "Use another account" → one click, then `cs_sso_login` when the
    email field appears;
  * `{kind:"login"}` on a password page → `cs_sso_login`;
  * a user click cancels;
  * iframes do nothing.
* **Docs:** autofill.md, security-model.md, threat-model.md,
  security-review.md; CLAUDE.md §25 note.

## 8. Out of scope

* Phone prompts, security keys or passkeys on the provider page: stop, the
  user finishes.
* Providers' "stay signed in?" or recovery screens: stop.
* Creating the provider login if the vault has none.

# Automatic sign-in — Design

Status: proposed, 2026-09-26.
Amends CLAUDE.md rule #6 ("never autofill without explicit user
interaction") for one bounded case: after the user picks a login, HavenKeys
may finish that one sign-in (press the button, fill later steps and the
TOTP code) without further clicks. See §7.

> This software has not undergone an independent security audit.

## 1. Goal

After the user picks a login in the in-page menu (or **Fill** in the popup),
HavenKeys fills it **and signs in**:

1. It presses the site's sign-in button.
2. It follows multi-step flows. Example (Amazon): fill email → press
   *Continue* → the password page loads → fill password → press *Sign in*.
3. If the login has TOTP and the site then asks for a one-time code, it
   fills the current code and presses *Verify*.

It must be conservative: when unsure, it fills without pressing, or stops.
Pressing the wrong thing is worse than not pressing.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Setting | Global vault setting `auto_sign_in`, **default on** | Off by default; extension-local option |
| Per-site escape hatch | Per-login switch `auto_sign_in` in the encrypted item payload, **default on** | Global only; per-site list in settings |
| Who decides auto-submit is allowed | Rust: `autoSubmit = settings.auto_sign_in && item.auto_sign_in`, returned with each fill | Extension reads settings and decides |
| Where the flow state lives | Background worker memory (a "sign-in run" per tab) | Rust-side run tracking (not a real boundary against a compromised extension, which can call `fill_item` anyway); content-script state (lost on navigation) |
| Run binding | The exact **origin** of the page where the user picked | Registrable domain (would allow a no-click fill on another subdomain) |
| Retries | Never. Each step is filled and pressed at most once | Retry on failure |
| Run lifetime | 2 minutes from the pick; 30 s to find each next step | — |

## 3. The sign-in run

### 3.1 Start

The user picks a login from the menu or the popup. The background calls
`fill_item` as today. The result now carries `autoSubmit` (§6.2).

* `autoSubmit: false` → exactly today's behaviour. No run.
* `autoSubmit: true` → the background creates a run in memory:

```text
run = {
  tabId,
  itemId,
  origin,        // scheme + host + port of the frame the user picked in
  frameId,       // the frame that holds the login (0 for the top frame)
  step,          // current step: "username" | "password" | "otp"
  hasTotp,       // from the Match the menu offered
  done: Set<step>,
  expires,       // pick time + 2 minutes
}
```

One run per tab. A new pick in the tab replaces the run. Locking the vault
(the bridge's lock event, or a `Locked` answer) drops every run. Nothing is
persisted. The run holds no secrets.

### 3.2 Steps

Forward only:

```text
username  →  password  →  otp  →  (end)
```

The first step is what the picked group shows. A group with username and
password is the `password` step: both are filled, then submitted. A
username-only group is the `username` step. A password-only group is the
`password` step. The `otp` step exists only when `hasTotp` is true.

After each step the content script fills, presses (§5) and, unless the run
is finished, watches for the next step (§4).

### 3.3 Continue

The content script sends `cs_run_step { kind: "password" | "otp" }` when it
sees the next step's field. The background accepts it only if all hold:

* a live run exists for the sender's tab (`sender.tab.id`, not message
  contents);
* the sender frame's origin (from `sender.url`) equals `run.origin`;
* `kind` comes after `run.step` and is not in `run.done`;
* for `otp`, `run.hasTotp` is true.

It then calls `fill_item` (password step) or `get_totp` (otp step) with the
frame's URL and top URL from the browser's sender data, so Rust re-checks
the origin binding on every step, and sends `bg_fill` to that frame with
`submit: true` and `next` (the step after this one, or `null`). If Rust
returns `autoSubmit: false` (the user changed a setting mid-run), the value
is filled without pressing and the run ends.

A password step fills only the password field, even if a username field is
also present again.

### 3.4 End

The run ends (background sends `bg_run_end` to the tab's frames; the
content script stops watching) when:

* the last step was submitted (the `otp` step, or the `password` step when
  `hasTotp` is false);
* any stop condition in §4.3 fires;
* 2 minutes have passed since the pick;
* the user picks another login in the tab;
* the tab closes or navigates to another origin (the content script's
  `cs_ready` from a different origin in the run's frame ends it);
* the vault locks.

## 4. Detecting the next step

### 4.1 Two cases

**The page navigates** (Amazon's email → password). On load, the content
script sends `cs_ready`. Today only the top frame does; now every frame
does. The background replies `{ watch: "password" | "otp" }` only when the
sender's tab has a live run, the frame is the run's frame, and its origin is
`run.origin`. Otherwise it replies as today. This is a message inside the
browser; the desktop is not contacted.

**The page re-renders without navigating** (SPAs). The `bg_fill` that
carried `submit: true` also carried `next`. After pressing, the content
script starts watching for `next` at once.

### 4.2 Watching (`autofill/watch.ts`)

1. Check once immediately with the bounded page lookup the popup already
   uses (`findLoginGroup` for `password`, `findOtpGroup` for `otp`; at most
   200 candidates, classified by the existing scorer). For `password`, the
   group must contain a `password` or `current-password` field.
2. If not found, observe `document` with one `MutationObserver`:
   `childList` + `subtree`, plus attributes filtered to `class`, `style`,
   `hidden`, `disabled`, `type` (sites often reveal a pre-rendered field by
   toggling a class). Mutations are debounced: at most one check per 150 ms.
3. **Stability:** a field counts only when the same connected, visible,
   enabled element is found in two consecutive checks (about 300 ms apart).
4. On success: disconnect, send `cs_run_step(kind)`.
5. Disconnect after 30 s with nothing found (and tell the background, which
   ends the run), on `bg_run_end`, and on `pagehide`.

No watching ever happens outside a run.

### 4.3 Stop conditions

Checked by the content script during watching and before each press. Any of
them ends the run; nothing more is filled or pressed.

| Condition | Detected by | Effect |
|---|---|---|
| The user takes over | A trusted `keydown`, `input` or `pointerdown` on the page during the run | End (so Esc ends it) |
| A step comes back | The field of an already-submitted step is found again (password after its submit: wrong password; OTP after its submit: wrong code) | End, no fill |
| Challenge on the page | A visible iframe whose parsed URL host is `www.google.com`/`www.recaptcha.net` with a `/recaptcha/` path, `*.hcaptcha.com`, or `challenges.cloudflare.com`; or a visible `.g-recaptcha`, `.h-captcha`, `.cf-turnstile` element in the group's root | Fill the current step, do not press, end |
| OTP without TOTP | `watch: "otp"` is never requested when `hasTotp` is false; an OTP field appearing then simply ends the run at the password step | End |
| Nothing appears | 30 s per step, 2 min overall | End |
| Context gone | Origin change, lock, tab closed, new pick | End (§3.4) |

## 5. Pressing the button (`autofill/submit.ts`)

### 5.1 Candidates

`button`, `input[type=submit]`, `input[type=image]`, `[role=button]` inside
the group's root (its `<form>`, or the container from `groupRoot`), at most
30, in document order. Kept only if visible (`isRendered`), not `disabled`,
not `aria-disabled="true"`.

### 5.2 Scoring

| Signal | Score |
|---|---|
| The submit button of the filled field's own form (`type=submit`, or `button` with no type, inside `field.form`) | +60 |
| Label fits the step. `username`: continue, next, proximo, continuar, avancar, seguinte. `password`: sign in, log in, login, signin, entrar, acessar, iniciar sesion. `otp`: verify, confirm, submit, verificar, confirmar, enviar | +50 |
| Any existing `SUBMIT_WORDS` word | +20 |
| After the last filled field in document order | +10 |
| Negative words: forgot, reset, create account, sign up, register, cadastrar, cancel, cancelar, back, voltar, resend, reenviar, show, mostrar, another, outra, passkey, "with google/apple/facebook/microsoft/github", "com google/apple/…" | disqualifies |

Label text is the element's `textContent`, `aria-label`, `title` and (for
inputs) `value`, normalized with `autofill/text.ts` and bounded to
`MAX_HINT_CHARS`, compared as whole words like field keywords.

**Ambiguity rule:** press only if the best candidate scores ≥ 60 **and**
beats the runner-up by ≥ 20. Otherwise the fields stay filled and the user
presses, which is today's behaviour. The run then ends.

### 5.3 Pressing

1. Wait for the button to become enabled: re-check every 100 ms, up to 1 s
   (many sites enable it only once the input validates). Still disabled →
   do not press, end the run.
2. Re-check the stop conditions (§4.3).
3. If the button belongs to a form (`button.form`), call
   `form.requestSubmit(button)`, so the site's validation and `submit`
   handlers run as for a click. Otherwise `button.click()`.
4. **OTP auto-submit:** before pressing in the `otp` step, wait 500 ms. If
   the OTP field is gone (disconnected or hidden) or the page is unloading,
   skip the press. The site submitted on its own.

Never synthetic key events, never Enter. Sites that reject scripted clicks
(`isTrusted` checks) simply do not react. The run then times out or the
user takes over.

### 5.4 Interaction with save-login

Values come from the vault, so `valueSource` is `"vault"` and
`readSubmission` offers no save prompt. The scripted press is not a trusted
click, so the click-based capture does not fire; a form `submit` event does
fire and is ignored by the same rule.

## 6. Settings, protocol, UI

### 6.1 Rust

* `Settings.auto_sign_in: bool`, `#[serde(default = "default_true")]`,
  `true` in `Default`.
* Login payload: `auto_sign_in: bool`, `#[serde(default = "default_true")]`,
  inside the encrypted item payload. Existing items read as on.
* `VaultService::fill_for_page` and `totp_for_page` return
  `auto_submit = settings.auto_sign_in && item.auto_sign_in` alongside the
  values. The existing checks (`Locked`, `Denied` for a non-matching origin,
  secure notes never served) are unchanged and come first.
* The desktop app's item editor reads and writes the item field through the
  existing item commands.

### 6.2 Protocol (`packages/protocol`)

* `fill_item` result: `{ type, username, password, autoSubmit: boolean }`.
* `get_totp` result: `{ type, code, period, secondsRemaining, autoSubmit: boolean }`.
* The parser requires `autoSubmit` to be a boolean; anything else is
  rejected like every other malformed result.
* No new native requests.

### 6.3 Extension messages (`messaging/inline.ts`), as implemented

All validated by the existing strict parsers; unknown or malformed messages
are dropped.

| Message | Direction | Content |
|---|---|---|
| `cs_ready` (reply, `ReadyReply`) | background → content | adds `watch: "password" \| "otp" \| null` |
| `bg_fill` | background → content | adds `submit: boolean` and `totp: boolean` (an OTP step follows the password step) |
| `bg_fill` reply (`FillReply`) | content → background | `{ filled: number; pressing: "username" \| "password" \| "otp" \| null }` |
| `cs_run_step` | content → background | `{ kind: "password" \| "otp" }` |
| `cs_run_stop` | content → background | `{}` (stop condition or timeout) |
| `bg_run_end` | background → content | `{}` |

The design above had `bg_fill` carry `next: "password" | "otp" | null`, computed
by the background. The implementation instead has `bg_fill`'s reply
(`FillReply.pressing`) tell the background which step it just pressed
(`"username" | "password" | "otp"`, or `null` when nothing was pressed): the
background does not itself know which step a picked group turned out to be
until the content script classifies and fills it (a group with only a
username field is a `username` step; one with a password field is a
`password` step, even when a username field is also present). The background
then derives the next step itself (`signin-run.ts` `nextStep`) from the
step just pressed and `hasTotp`, and starts or continues the run from there;
`totp` on `bg_fill` still tells the content script whether an OTP step
should be watched for after a password press.

### 6.4 Desktop UI

* Settings → *Browser extension*: toggle **"Sign in automatically after
  filling"**. Note: "Presses the sign-in button and continues through
  email, password and 2FA steps. Sign-ins that span several pages need
  in-page suggestions."
* Login editor: checkbox **"Sign in automatically on this site"**, default
  checked. Disabled with the hint "Turned off in Settings" when the global
  setting is off.

### 6.5 Popup-only mode

Without the in-page suggestions permission, the popup injects the content
script via `activeTab`, which the browser revokes when the tab navigates.
A run then covers only the current page: single-page forms and SPA steps.
Steps on a new page need in-page suggestions. Documented in `autofill.md`.

## 7. Security

* **Only a trusted pick starts a run.** The pick is the existing guarded
  click in the extension's menu frame (or the popup). Page script cannot
  start, extend or re-target a run: `cs_run_step` is accepted only from the
  run's tab, frame and origin, only for the next step, once.
* **Rust still checks every value.** Each step's password or code comes
  from `fill_item` / `get_totp` against the frame and top URLs the browser
  reports. `autoSubmit` is computed in Rust, so the extension cannot enable
  it for a login the user switched off.
* **Origin binding.** Continuation never crosses origins, even within a
  whole-site rule. A redirect to another subdomain (for example a
  user-content host) gets no no-click fill; the menu still works there
  manually.
* **No secrets in run state.** Item ID, origin, frame, step, timestamps.
  Memory only.
* **Accepted risk (new `security-review.md` finding).** For up to 2 minutes
  after a pick, a page on that same origin receives the password and the
  current TOTP code without further clicks. Script on that origin could
  already obtain the password after the single manual pick; the new part is
  that the OTP no longer needs its own click. Bounded by origin binding,
  forward-only single-use steps, the 2-minute window, and the global and
  per-login off switches. Same class as PK22 (automatic passkey upgrade).
* **Wrong button.** The ambiguity rule and negative words make a
  mis-press unlikely; a mis-press on the matched origin cannot leak
  credentials elsewhere.

## 8. Code layout

| File | Role |
|---|---|
| `crates/havenkeys-core/src/model.rs` | `Settings.auto_sign_in`, login `auto_sign_in` |
| `crates/havenkeys-core/src/vault.rs` (or where `fill_for_page` / `totp_for_page` live) | `auto_submit` in results |
| native host / bridge result mapping | `autoSubmit` on the wire |
| `packages/protocol/src/index.ts` | result types and parser |
| `apps/desktop/src/lib/types.ts`, `views/SettingsView.tsx`, login editor | toggle and checkbox |
| `apps/extension/src/autofill/submit.ts` | button scoring and pressing (new, pure) |
| `apps/extension/src/autofill/watch.ts` | next-step watcher (new) |
| `apps/extension/src/background/signin-run.ts` | run state and transitions (new, no `chrome.*`) |
| `apps/extension/src/background/inline-handler.ts`, `popup-handler.ts` | start runs on picks, route `cs_run_step` / `cs_run_stop` |
| `apps/extension/src/messaging/inline.ts` | message types and parsers |
| `apps/extension/src/content/index.ts` | wiring only |

## 9. Testing

**Rust**
* `autoSubmit` for all four combinations of global and item flags.
* Old settings and old items without the field read as on.
* `Locked` and a non-matching origin are still denied before `autoSubmit`
  is considered.

**Protocol**
* `autoSubmit` missing, non-boolean → result rejected. Fuzz corpus updated.

**`submit.ts`**
* Picks *Continue* over *Create account*; *Sign in* over *Sign in with
  Google* and *Forgot password?*.
* Ambiguous (two equal candidates) → no press.
* Disabled button that enables within 1 s → pressed; never enables → not.
* Form button → `requestSubmit` called; form-less → `click` called.
* OTP field removed within 500 ms → no press.

**`watch.ts`**
* Field present at start; field inserted later (SPA); field revealed by a
  class change.
* A field that appears for one check and disappears → not reported.
* 30 s timeout → stop reported.
* A mutation storm (5,000 nodes) stays bounded (attack 7).

**`signin-run.ts`**
* Full Amazon-style flow: username → (navigation, `cs_ready` → `watch`)
  → password → otp → end.
* `hasTotp` false → ends after the password step; `watch: "otp"` never sent.
* Password step reported again after submit → ends, no second fill.
* Ends on origin change, user input (`cs_run_stop`), lock, new pick, expiry.
* `cs_run_step` from another tab, another frame, another origin, out of
  order, or repeated → ignored.
* `autoSubmit: false` → no run.

**Security regressions**
* A page cannot start a run (no pick → `cs_run_step` ignored).
* A run started on `a.example.com` does not fill on `b.example.com`.

## 10. Docs

* `docs/autofill.md`: new "Automatic sign-in" section (flow, stop
  conditions, button scoring, popup-only limit), and Limitations entries
  (host-hopping flows, `isTrusted`-checking sites, CAPTCHAs).
* `docs/security-model.md`, `docs/threat-model.md`: the run, its binding,
  the accepted risk.
* `docs/security-review.md`: new accepted finding (§7).
* `docs/native-messaging.md`: `autoSubmit` on `fill_item` / `get_totp`.
* CLAUDE.md: amendment note under §25 pointing to this spec.

## 11. Out of scope

* Retrying a failed step.
* SMS/email codes, security questions, account pickers.
* Following sign-ins across origins.
* Ticking "remember me" checkboxes.
* A visible "signing in…" indicator.

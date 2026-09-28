# Extension on by default; suggestions only hide the field menu — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Site access is granted at install so save/update prompts and passkeys work without setup; "Suggestions in login fields" becomes a stored preference (default on) that only shows/hides the field icon and menu.

**Architecture:** Host patterns move to required `host_permissions`; `registration.ts` keeps registering scripts for whatever is granted. A tiny `shared/prefs.ts` wraps one boolean in `chrome.storage.local`. The background's `openMenu` refuses menus when it is off (authoritative); the content script also skips the icon and menu requests and follows changes live. The options page gets a preference toggle plus a site-access status with an Allow button.

**Tech Stack:** TypeScript, WebExtension MV3 (Chrome ≥120, Firefox ≥128), vitest + jsdom.

**Spec:** `docs/superpowers/specs/2026-09-28-extension-defaults-design.md`

## Global Constraints

- Storage key: `inlineSuggestions`; absent, unreadable or non-boolean → on. Only this boolean is stored.
- New permission: `storage` only. `host_permissions: ["https://*/*", "http://*/*"]`; no `optional_host_permissions`.
- Desktop `browser_integration` default stays `false` (security-review P6).
- Preference never affects: `cs_submit`/save prompt, `cs_ready`, `cs_run_step`/`cs_run_stop`, passkey messages, popup fill.
- Every user-visible string exists in `i18n/en.ts` and `i18n/pt-BR.ts`; DOM built with `textContent` only.
- No co-author trailers in commits (user memory).

## Review Focus

1. Preference turned off while a menu is open → menu closes and icon disappears without reload (Task 4 test).
2. Storage unavailable (content script injected where `chrome.storage` is missing, or `get` throws) → treated as on, never breaks the page (Task 1 test).
3. Explicit icon request (`explicit: true`) with preference off → still no menu (Task 3 test).
4. User withdrew site access in the browser → options shows Allow, toggle still writes the preference (Task 5 test).
5. Preference off → save prompt still appears for a new and for a changed password (Task 3 test).

---

### Task 1: Preference module

**Files:**
- Create: `apps/extension/src/shared/prefs.ts`
- Test: `apps/extension/src/shared/prefs.test.ts`

**Interfaces:**
- Produces: `getInlineSuggestions(): Promise<boolean>`, `setInlineSuggestions(on: boolean): Promise<void>`, `onInlineSuggestionsChanged(cb: (on: boolean) => void): void`, `INLINE_SUGGESTIONS_KEY = "inlineSuggestions"`.

- [ ] **Step 1: Write the failing test**

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { getInlineSuggestions, onInlineSuggestionsChanged, setInlineSuggestions } from "./prefs";

type Changed = (changes: Record<string, { newValue?: unknown }>, area: string) => void;

function fakeStorage(initial: Record<string, unknown> = {}) {
  const data = { ...initial };
  let changed: Changed | null = null;
  vi.stubGlobal("chrome", {
    storage: {
      local: {
        get: async (k: string) => (k in data ? { [k]: data[k] } : {}),
        set: async (o: Record<string, unknown>) => Object.assign(data, o),
      },
      onChanged: { addListener: (l: Changed) => (changed = l) },
    },
  });
  return { data, fire: (c: Record<string, { newValue?: unknown }>, area = "local") => changed?.(c, area) };
}

afterEach(() => vi.unstubAllGlobals());

describe("inline suggestions preference", () => {
  it("is on when absent, off only when stored false", async () => {
    fakeStorage();
    expect(await getInlineSuggestions()).toBe(true);
    fakeStorage({ inlineSuggestions: false });
    expect(await getInlineSuggestions()).toBe(false);
    fakeStorage({ inlineSuggestions: "no" });
    expect(await getInlineSuggestions()).toBe(true);
  });

  it("is on when storage is missing or fails", async () => {
    vi.stubGlobal("chrome", {});
    expect(await getInlineSuggestions()).toBe(true);
    vi.stubGlobal("chrome", { storage: { local: { get: async () => { throw new Error("x"); } } } });
    expect(await getInlineSuggestions()).toBe(true);
  });

  it("writes only the boolean", async () => {
    const { data } = fakeStorage();
    await setInlineSuggestions(false);
    expect(data).toEqual({ inlineSuggestions: false });
  });

  it("reports changes to the local area only", () => {
    const { fire } = fakeStorage();
    const seen: boolean[] = [];
    onInlineSuggestionsChanged((on) => seen.push(on));
    fire({ inlineSuggestions: { newValue: false } });
    fire({ inlineSuggestions: { newValue: true } });
    fire({ inlineSuggestions: {} }); // removed → on
    fire({ inlineSuggestions: { newValue: false } }, "sync");
    fire({ other: { newValue: false } });
    expect(seen).toEqual([false, true, true]);
  });
});
```

- [ ] **Step 2: Run** `pnpm --filter @havenkeys/extension exec vitest run src/shared/prefs.test.ts` — expect FAIL (module missing).

- [ ] **Step 3: Implement**

```ts
// The extension's one stored preference: whether the field icon and the
// menu under login fields are shown. A non-sensitive boolean in
// chrome.storage.local; save prompts, passkeys and popup fills never read
// it. Absent, unreadable or not a boolean means on.

export const INLINE_SUGGESTIONS_KEY = "inlineSuggestions";

const isOn = (v: unknown): boolean => v !== false;

export async function getInlineSuggestions(): Promise<boolean> {
  try {
    const got = await globalThis.chrome?.storage?.local?.get(INLINE_SUGGESTIONS_KEY);
    return isOn(got?.[INLINE_SUGGESTIONS_KEY]);
  } catch {
    return true;
  }
}

export async function setInlineSuggestions(on: boolean): Promise<void> {
  await globalThis.chrome?.storage?.local?.set({ [INLINE_SUGGESTIONS_KEY]: on });
}

export function onInlineSuggestionsChanged(cb: (on: boolean) => void): void {
  globalThis.chrome?.storage?.onChanged?.addListener((changes, area) => {
    if (area === "local" && INLINE_SUGGESTIONS_KEY in changes) cb(isOn(changes[INLINE_SUGGESTIONS_KEY]?.newValue));
  });
}
```

- [ ] **Step 4: Run** the test — expect PASS.
- [ ] **Step 5: Commit** `feat(extension): stored preference for in-page suggestions`

### Task 2: Required site access in the manifest

**Files:**
- Modify: `apps/extension/manifest/base.json` (permissions, host permissions)
- Modify: `apps/extension/src/background/registration.ts:1-10` (header comment)
- Test: `apps/extension/src/i18n/manifest.test.ts` (new `describe`)

- [ ] **Step 1: Failing test** — append to `manifest.test.ts`:

```ts
describe("manifest permissions", () => {
  it("has site access from install and asks for nothing else new", () => {
    expect(base.permissions).toEqual(["nativeMessaging", "activeTab", "scripting", "storage"]);
    expect((base as Record<string, unknown>).host_permissions).toEqual(["https://*/*", "http://*/*"]);
    expect("optional_host_permissions" in base).toBe(false);
  });
});
```

- [ ] **Step 2: Run** `pnpm --filter @havenkeys/extension exec vitest run src/i18n/manifest.test.ts` — FAIL.
- [ ] **Step 3: Implement** — in `base.json` replace the `permissions` and `optional_host_permissions` blocks with:

```json
  "permissions": [
    "nativeMessaging",
    "activeTab",
    "scripting",
    "storage"
  ],
  "host_permissions": [
    "https://*/*",
    "http://*/*"
  ],
```

Replace the first paragraph of the `registration.ts` header with:

```ts
// Site access is requested at install (host_permissions), so the content
// script and the passkey scripts are registered by default. They are
// registered only for the host patterns actually granted: the user can
// still narrow or withdraw access in the browser's own site-access
// controls, and the scripts follow. Without any grant the extension works
// from the toolbar popup alone, with activeTab.
```

- [ ] **Step 4: Run** the test — PASS.
- [ ] **Step 5: Commit** `feat(extension): site access granted at install`

### Task 3: Background refuses menus when suggestions are off

**Files:**
- Modify: `apps/extension/src/background/inline-handler.ts` (`InlineDeps`, `openMenu`)
- Modify: `apps/extension/src/background/index.ts:62-70` (wire the dep)
- Test: `apps/extension/src/background/inline-handler.test.ts`

**Interfaces:**
- Consumes: `getInlineSuggestions` (Task 1).
- Produces: `InlineDeps.suggestionsOn?(): Promise<boolean>` (absent → on).

- [ ] **Step 1: Failing tests** — add a `describe("suggestions preference")` block:

```ts
describe("suggestions preference", () => {
  const off = { suggestionsOn: async () => false };

  it("opens no menu of any kind when off, even from the field icon, and asks the desktop nothing", async () => {
    const { h, requests } = setup(defaultAnswer, off);
    for (const kind of ["login", "otp", "new_password"] as const) {
      expect(await h.handleContent(frame(), { type: "cs_open_menu", kind })).toEqual({ ok: false });
      expect(await h.handleContent(frame(), { type: "cs_open_menu", kind, explicit: true })).toEqual({ ok: false });
    }
    expect(requests).toEqual([]);
  });

  it("still offers to save a new login and to update a changed password when off", async () => {
    const { h, sent } = setup(defaultAnswer, off);
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "typed" });
    expect(await h.handleInline(1, { type: "save_state", token: T1 })).toMatchObject({ ok: true, value: { action: "add" } });
    expect(sent[0]?.msg).toEqual({ type: "bg_show_save", token: T1 });

    const upd = setup((r) => (r.type === "check_login" ? { type: "check_login", action: "update", itemId: GH } : defaultAnswer(r)), off);
    await upd.h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "changed" });
    expect(await upd.h.handleInline(1, { type: "save_state", token: T1 })).toMatchObject({ ok: true, value: { action: "update" } });
  });

  it("still reshows a pending save prompt on the next page when off", async () => {
    const { h } = setup(defaultAnswer, off);
    await h.handleContent(frame(), { type: "cs_submit", username: "octo", password: "pw" });
    expect(await h.handleContent(frame(), { type: "cs_ready" })).toMatchObject({ saveToken: T1 });
  });
});
```

- [ ] **Step 2: Run** `pnpm --filter @havenkeys/extension exec vitest run src/background/inline-handler.test.ts` — the first test FAILS (menus open).
- [ ] **Step 3: Implement** — in `InlineDeps` add:

```ts
  /** Whether the field menu may open (the options page preference). Absent: on. */
  suggestionsOn?(): Promise<boolean>;
```

First lines of `openMenu`:

```ts
    // The user hid the menu under login fields; saving and passkeys go on.
    if (deps.suggestionsOn && !(await deps.suggestionsOn())) return { ok: false };
```

In `index.ts` add `import { getInlineSuggestions } from "../shared/prefs";` and `suggestionsOn: getInlineSuggestions,` to `createInlineHandler({...})`.

- [ ] **Step 4: Run** the file — all PASS.
- [ ] **Step 5: Commit** `feat(extension): the background opens no field menu when suggestions are off`

### Task 4: Content script hides icon and menu when off

**Files:**
- Modify: `apps/extension/src/content/index.ts` (`start`, `maybeOpen`, `showIcon`, focused-at-load)
- Test: `apps/extension/src/content/content.test.ts`

**Interfaces:** Consumes `getInlineSuggestions`, `onInlineSuggestionsChanged` (Task 1).

- [ ] **Step 1: Failing test** — in `beforeAll`'s fake `chrome` add

```ts
    storage: {
      local: { get: async () => ({}) },
      onChanged: { addListener: (l: StorageListener) => (storageListener = l) },
    },
```

with `type StorageListener = (c: Record<string, { newValue?: unknown }>, area: string) => void; let storageListener: StorageListener | null = null;` at file top, and add:

```ts
  it("hides the field icon and any open menu while suggestions are off, without a reload", async () => {
    const setPref = (v: boolean) => storageListener?.({ inlineSuggestions: { newValue: v } }, "local");
    const iconEl = () => document.documentElement.querySelector(":scope > div[title='HavenKeys']");
    const pw = field("pw");
    pw.focus();
    expect(iconEl()).not.toBeNull();
    setPref(false);
    expect(iconEl()).toBeNull();
    pw.blur();
    pw.focus();
    expect(iconEl()).toBeNull();
    setPref(true);
    expect(iconEl()).not.toBeNull(); // the focused field gets it back
    pw.blur();
  });
```

- [ ] **Step 2: Run** `pnpm --filter @havenkeys/extension exec vitest run src/content/content.test.ts` — FAIL.
- [ ] **Step 3: Implement** in `content/index.ts`:
  - import `{ getInlineSuggestions, onInlineSuggestionsChanged } from "../shared/prefs"`.
  - in `start()` state: `/** The options-page preference; false until read so no icon flashes. */ let suggestions = false;`
  - first line of `maybeOpen`: `if (!suggestions) return;` (merged into the existing guard).
  - first line of `showIcon`: `if (!suggestions) return hideIcon();`
  - replace the "focused before we loaded" block with:

```ts
  // A field the page focused before we loaded gets its icon (not a menu),
  // once we know suggestions are on.
  function focusedField(): HTMLInputElement | null {
    const f = deepActiveElement();
    return f instanceof HTMLInputElement ? f : null;
  }
  function applySuggestions(on: boolean): void {
    suggestions = on;
    if (!on) {
      closeMenu(true);
      hideIcon();
      return;
    }
    const f = focusedField();
    if (f) showIcon(f);
  }
  onInlineSuggestionsChanged(applySuggestions);
  void getInlineSuggestions().then(applySuggestions);
```

- [ ] **Step 4: Run** the file — all PASS (existing icon test still passes: the read resolves before tests run).
- [ ] **Step 5: Commit** `feat(extension): content script follows the suggestions preference`

### Task 5: Options page and popup copy

**Files:**
- Modify: `apps/extension/src/options/options.html`, `apps/extension/src/options/options.ts`
- Modify: `apps/extension/src/i18n/en.ts` (`options`, `popup.offer`), `apps/extension/src/i18n/pt-BR.ts`
- Test: `apps/extension/src/options/options.test.ts` (new, jsdom)

**Interfaces:** Consumes Task 1, `INLINE_ORIGINS`/`grantedOrigins` from `registration.ts`.

- [ ] **Step 1: Strings.** Replace `options` in `en.ts` with:

```ts
  options: {
    pageTitle: "HavenKeys settings",
    subtitle: "Browser extension settings",
    suggestionsTitle: "Suggestions in login fields",
    toggleLabel: "Show my logins under login fields",
    suggestionsOn: "On. Clicking a login field shows your matching logins under it.",
    suggestionsOff: "Off. Use the HavenKeys button in the toolbar to fill.",
    notes: [
      "When this is on, clicking a username, password or one-time-code field on a website shows your matching HavenKeys logins right under the field, and sign-up fields offer a generated password. Nothing is filled until you pick a login, and logins are only offered on the websites they are saved for.",
      "Offers to save new or changed passwords, and passkeys, work with this off.",
    ],
    turnOn: "Turn on",
    turnOff: "Turn off",
    accessTitle: "Site access",
    accessLabel: "Save prompts and passkeys",
    accessOn: "On. HavenKeys can offer to save logins and handle passkeys on the websites you visit.",
    accessHttps: "On for secure (https) websites.",
    accessOff: "Off. HavenKeys cannot offer to save logins or handle passkeys. Filling from the toolbar button still works.",
    allow: "Allow",
    accessNote:
      "The extension reads login fields when you interact with them and never sends page contents anywhere; the desktop app decides which logins a website may use. You can limit site access in your browser's extension settings.",
    withoutTitle: "Without site access",
    /** Split around the popup's "Fill" button label, which is shown emphasised. */
    withoutBefore: "Click the HavenKeys button in the toolbar and choose ",
    withoutAfter: ". The extension then has access only to the tab you clicked on, and only until you leave the page.",
  },
```

and `popup.offer` with:

```ts
    offer: {
      title: "Site access is off",
      body: "Allow it to get offers to save logins and to use passkeys with HavenKeys.",
      turnOn: "Allow",
      turnOnTitle: "Let HavenKeys run on the websites you visit",
      doneTitle: "Site access is on",
      doneBody: "Reload open tabs to use it there.",
    },
```

pt-BR equivalents:

```ts
  options: {
    pageTitle: "Configurações do HavenKeys",
    subtitle: "Configurações da extensão do navegador",
    suggestionsTitle: "Sugestões nos campos de login",
    toggleLabel: "Mostrar meus logins abaixo dos campos de login",
    suggestionsOn: "Ativado. Ao clicar em um campo de login, seus logins para o site aparecem abaixo dele.",
    suggestionsOff: "Desativado. Use o botão do HavenKeys na barra de ferramentas para preencher.",
    notes: [
      "Com esta opção ativada, ao clicar em um campo de usuário, senha ou código de verificação em um site, seus logins do HavenKeys para esse site aparecem logo abaixo do campo, e os campos de cadastro oferecem uma senha gerada. Nada é preenchido até você escolher um login, e cada login só é oferecido nos sites para os quais foi salvo.",
      "Salvar senhas novas ou alteradas e usar chaves de acesso continuam funcionando com esta opção desligada.",
    ],
    turnOn: "Ativar",
    turnOff: "Desativar",
    accessTitle: "Acesso aos sites",
    accessLabel: "Salvar logins e chaves de acesso",
    accessOn: "Ativado. O HavenKeys pode oferecer salvar logins e cuidar de chaves de acesso nos sites que você visita.",
    accessHttps: "Ativado para sites seguros (https).",
    accessOff: "Desativado. O HavenKeys não pode oferecer salvar logins nem cuidar de chaves de acesso. Preencher pelo botão da barra de ferramentas continua funcionando.",
    allow: "Permitir",
    accessNote:
      "A extensão lê os campos de login quando você interage com eles e nunca envia o conteúdo das páginas a lugar nenhum; o app para computador decide quais logins cada site pode usar. Você pode limitar o acesso aos sites nas configurações de extensões do navegador.",
    withoutTitle: "Sem acesso aos sites",
    withoutBefore: "Clique no botão do HavenKeys na barra de ferramentas e escolha ",
    withoutAfter: ". A extensão passa a ter acesso só à aba em que você clicou, e só até você sair da página.",
  },
```

```ts
    offer: {
      title: "O acesso aos sites está desativado",
      body: "Permita para receber ofertas de salvar logins e usar chaves de acesso com o HavenKeys.",
      turnOn: "Permitir",
      turnOnTitle: "Deixar o HavenKeys rodar nos sites que você visita",
      doneTitle: "Acesso aos sites ativado",
      doneBody: "Recarregue as abas abertas para usá-lo nelas.",
    },
```

(Keep each file's existing comment/style; pt-BR must type-check against en's shape.)

- [ ] **Step 2: HTML.** Replace the body sections of `options.html` between `</header>` and `</main>` with:

```html
      <h2 id="suggestions-title" class="group-title"></h2>
      <section class="group">
        <div class="row">
          <div class="row-main">
            <strong id="toggle-label"></strong>
            <p id="status" class="status" aria-live="polite"></p>
          </div>
          <button id="toggle" class="btn" type="button" hidden></button>
        </div>
      </section>
      <div id="notes"></div>

      <h2 id="access-title" class="group-title"></h2>
      <section class="group">
        <div class="row">
          <div class="row-main">
            <strong id="access-label"></strong>
            <p id="access-status" class="status" aria-live="polite"></p>
          </div>
          <button id="allow" class="btn primary" type="button" hidden></button>
        </div>
      </section>
      <p id="access-note" class="note"></p>

      <h2 id="without-title" class="group-title"></h2>
      <section class="group">
        <p id="without" class="group-text"></p>
      </section>
```

- [ ] **Step 3: Failing test** `src/options/options.test.ts`:

```ts
// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { beforeEach, describe, expect, it, vi } from "vitest";

let granted: string[] = [];
let stored: Record<string, unknown> = {};
const permCalls: string[] = [];

function install() {
  const html = readFileSync(new URL("./options.html", import.meta.url), "utf8");
  document.body.innerHTML = html.slice(html.indexOf("<main>"), html.indexOf("</main>") + 7);
  vi.stubGlobal("chrome", {
    permissions: {
      contains: async ({ origins }: { origins: string[] }) => origins.every((o) => granted.includes(o)),
      request: async () => (permCalls.push("request"), (granted = ["https://*/*", "http://*/*"]), true),
      remove: async () => (permCalls.push("remove"), true),
      onAdded: { addListener: () => undefined },
      onRemoved: { addListener: () => undefined },
    },
    storage: {
      local: {
        get: async (k: string) => (k in stored ? { [k]: stored[k] } : {}),
        set: async (o: Record<string, unknown>) => Object.assign(stored, o),
      },
      onChanged: { addListener: () => undefined },
    },
  });
}

const flush = () => new Promise((r) => setTimeout(r, 0));
const $ = (id: string) => document.getElementById(id) as HTMLElement;

beforeEach(() => {
  vi.resetModules();
  granted = ["https://*/*", "http://*/*"];
  stored = {};
  permCalls.length = 0;
});

describe("options page", () => {
  it("the suggestions switch writes the preference and never touches permissions", async () => {
    install();
    await import("./options");
    await flush();
    expect($("toggle").textContent).toBe("Turn off");
    $("toggle").click();
    await flush();
    expect(stored).toEqual({ inlineSuggestions: false });
    expect($("toggle").textContent).toBe("Turn on");
    expect(permCalls).toEqual([]);
    expect(($("allow") as HTMLButtonElement).hidden).toBe(true);
  });

  it("offers Allow only when site access was withdrawn", async () => {
    granted = [];
    install();
    await import("./options");
    await flush();
    expect(($("allow") as HTMLButtonElement).hidden).toBe(false);
    expect($("access-status").textContent).toContain("Off.");
    $("allow").click();
    await flush();
    expect(permCalls).toEqual(["request"]);
    expect(($("allow") as HTMLButtonElement).hidden).toBe(true);
  });
});
```

- [ ] **Step 4: Run** `pnpm --filter @havenkeys/extension exec vitest run src/options/options.test.ts` — FAIL.
- [ ] **Step 5: Implement** `options.ts`:

```ts
// Options page: the in-page suggestions preference (the field icon and the
// menu under login fields), and the site access that save prompts and
// passkeys need. Site access is granted at install; if the user withdrew
// it in the browser, Allow asks for it again. The background re-registers
// the content scripts when the grant changes (background/registration.ts).

import { INLINE_ORIGINS, grantedOrigins } from "../background/registration";
import { applyDocumentLang, t } from "../i18n";
import { getInlineSuggestions, onInlineSuggestionsChanged, setInlineSuggestions } from "../shared/prefs";

const status = document.getElementById("status") as HTMLElement;
const toggle = document.getElementById("toggle") as HTMLButtonElement;
const accessStatus = document.getElementById("access-status") as HTMLElement;
const allow = document.getElementById("allow") as HTMLButtonElement;
let on = true;

function text(id: string, value: string): void {
  (document.getElementById(id) as HTMLElement).textContent = value;
}

/** Static copy, filled from the message table (textContent only). */
function fillStatic(): void {
  applyDocumentLang();
  document.title = t.options.pageTitle;
  text("subtitle", t.options.subtitle);
  text("suggestions-title", t.options.suggestionsTitle);
  text("toggle-label", t.options.toggleLabel);
  text("access-title", t.options.accessTitle);
  text("access-label", t.options.accessLabel);
  text("access-note", t.options.accessNote);
  allow.textContent = t.options.allow;
  text("without-title", t.options.withoutTitle);
  const notes = document.getElementById("notes") as HTMLElement;
  notes.replaceChildren(
    ...t.options.notes.map((note) => {
      const p = document.createElement("p");
      p.className = "note";
      p.textContent = note;
      return p;
    }),
  );
  const fill = document.createElement("em");
  fill.textContent = t.popup.fill;
  (document.getElementById("without") as HTMLElement).replaceChildren(t.options.withoutBefore, fill, t.options.withoutAfter);
}

function showSuggestions(value: boolean): void {
  on = value;
  status.textContent = on ? t.options.suggestionsOn : t.options.suggestionsOff;
  status.className = on ? "status on" : "status";
  toggle.textContent = on ? t.options.turnOff : t.options.turnOn;
  toggle.className = on ? "btn" : "btn primary";
  toggle.hidden = false;
}

async function refreshAccess(): Promise<void> {
  const granted = await grantedOrigins();
  if (granted.length === 0) {
    accessStatus.textContent = t.options.accessOff;
    accessStatus.className = "status";
  } else {
    accessStatus.textContent = granted.length < INLINE_ORIGINS.length ? t.options.accessHttps : t.options.accessOn;
    accessStatus.className = "status on";
  }
  allow.hidden = granted.length > 0;
}

toggle.addEventListener("click", () => {
  const next = !on;
  void setInlineSuggestions(next).then(() => showSuggestions(next));
});

allow.addEventListener("click", () => {
  // permissions.request must run directly in the click handler.
  void chrome.permissions
    .request({ origins: [...INLINE_ORIGINS] })
    .catch(() => false)
    .then(refreshAccess);
});

fillStatic();
onInlineSuggestionsChanged(showSuggestions);
chrome.permissions.onAdded.addListener(() => void refreshAccess());
chrome.permissions.onRemoved.addListener(() => void refreshAccess());
void getInlineSuggestions().then(showSuggestions);
void refreshAccess();
```

Remove the now-unused keys from `en.ts`/`pt-BR.ts` (`statusOff`, `statusOn`, `statusHttps`). Check `grep -rn "statusOff\|statusHttps\|statusOn" apps/extension/src` is empty.

- [ ] **Step 6: Run** the options test, `src/i18n`, and `pnpm --filter @havenkeys/extension typecheck` — PASS.
- [ ] **Step 7: Commit** `feat(extension): options page separates suggestions from site access`

### Task 6: Documentation

**Files:** `CLAUDE.md` §18, `docs/security-model.md` §12, `docs/threat-model.md` §2, `docs/autofill.md` (Saving logins, suggestions intro), `docs/native-messaging.md` §2 step 6, `docs/security-review.md` (new section at the end), `README.md` (lines 20, 45–60), `apps/web/src/i18n/en.tsx` + `pt-BR.tsx` (permissions list, ~308, ~596), spec status → implemented.

- [ ] **Step 1:** CLAUDE.md, after §18's list, add:

```md
> Amended on 2026-09-28 by
> `docs/superpowers/specs/2026-09-28-extension-defaults-design.md`: site
> access (`https://*/*`, `http://*/*`) is requested at install so save
> prompts and passkeys work without setup. The menu under login fields is
> a separate preference, on by default. Desktop browser integration stays
> opt-in.
```

- [ ] **Step 2:** security-model §12: table row → `| \`https://*/*\`, \`http://*/*\` | yes (host_permissions) | Content script for save prompts and in-page suggestions, and the passkey scripts. Granted at install; the user can narrow or withdraw it in the browser, and the registered scripts follow the grant |`; add `| \`storage\` | yes | One boolean: whether in-page suggestions are shown. Nothing else is stored |`; replace the "Why not ask for everything at install" paragraph with the spec §9 reasoning (larger default surface accepted; scripts treat every page as hostile; desktop integration stays opt-in so nothing reaches the vault until the user turns it on); drop `storage` from "Not requested"; passkey-scripts paragraph "when the user grants host access" → "on granted hosts (all http/https by default)".
- [ ] **Step 3:** threat-model §2: "When in-page suggestions are on, a content script reads…" → "On granted hosts (by default every http/https page), a content script reads…".
- [ ] **Step 4:** autofill.md: state that saving logins and passkeys do not depend on the suggestions preference; the preference hides the field icon and menu (including the sign-up generator); read live from `chrome.storage.local`.
- [ ] **Step 5:** native-messaging §2 step 6 → "In-page suggestions are on by default; the extension's options page can turn them off (save prompts and passkeys keep working)."
- [ ] **Step 6:** security-review: append `## Extension site access on by default (2026-09-28)` with a row `| E1 | Info | Extension | Content and passkey scripts run on every http(s) page by default instead of after opt-in | Accepted (spec 2026-09-28 §9); desktop integration stays opt-in (P6) |` and a short paragraph including the Chrome update-disable consequence.
- [ ] **Step 7:** README and web copy: drop "opt-in" for suggestions/site access; web permissions list marks host access `optional: false` with a new `why`; add `storage`.
- [ ] **Step 8:** `pnpm --filter <web package> typecheck` (and its tests if present) — PASS. Commit `docs: extension site access on by default`.

### Task 7: Full verification

- [ ] `pnpm --filter @havenkeys/extension typecheck && pnpm --filter @havenkeys/extension test`
- [ ] `pnpm --filter @havenkeys/extension build` and check `dist/chrome/manifest.json` and `dist/firefox/manifest.json` contain `host_permissions` and `storage`.
- [ ] `grep -rn "optional_host_permissions" apps/extension` → only historical docs, none in source/manifest.
- [ ] Secret-leak review of the diff: only a boolean is stored; no logging added.

// Extension pages under a stubbed `chrome`: popup, in-page menu, save
// prompt, passkey card and options, each in every state worth seeing, at the
// size the browser gives them.

const TOKEN = "0123456789abcdef0123456789abcdef";
const ID1 = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const ID2 = "11111111-2222-4333-8444-555555555555";
const ID3 = "22222222-3333-4444-8555-666666666666";
const CRED1 = "AQEBAQEBAQEBAQEBAQEBAQ";
const CRED2 = "AgICAgICAgICAgICAgICAg";

// Frame geometry, as content/frames.ts sets it.
const MENU_HEADER = 34;
const MENU_ROW = 46;
const MENU_PADDING = 10;
const SAVE = { width: 340, height: 138 };
const PASSKEY = { width: 380, height: 180 };

/** Messages the background would send, in the page's language. */
const TEXT = {
  en: {
    hostUnavailable: "The HavenKeys native messaging host is not installed or failed to start.",
    integrationDisabled: "Browser integration is turned off in HavenKeys settings.",
    promptExpired: "This prompt has expired.",
    denied: "This item is not saved for this site.",
    offline: "HavenKeys is offline. The vault is read-only until it reconnects.",
  },
  "pt-BR": {
    hostUnavailable: "O host de mensagens nativas do HavenKeys não está instalado ou não conseguiu iniciar.",
    integrationDisabled: "A integração com o navegador está desativada nas configurações do HavenKeys.",
    promptExpired: "Esta solicitação expirou.",
    denied: "Este item não está salvo para este site.",
    offline: "O HavenKeys está offline. O cofre fica somente leitura até reconectar.",
  },
};

const ok = (value) => ({ ok: true, value });
const err = (message) => ({ ok: false, message });

const matches = [
  { id: ID1, title: "GitHub", username: "octocat@example.com", hasTotp: true, strength: "same_host" },
  {
    id: ID2,
    title: "GitHub — work account for the platform infrastructure team",
    username: "firstname.lastname.with-a-long-address@corporate-example.com",
    hasTotp: false,
    strength: "same_host",
  },
  { id: ID3, title: "GitHub (old)", username: null, hasTotp: false, strength: "same_host" },
];
const menuItems = matches.map(({ id, title, username }) => ({ id, title, username }));

function menuView(over) {
  return { state: "ready", kind: "login", site: "github.com", items: menuItems.slice(0, 2), passkeys: [], hint: null, ...over };
}

/**
 * Scenarios. `frame`: how the page is sized.
 *   popup   — 320 wide, as tall as its body;
 *   menu    — `width` wide, starting at the row estimate, then what menu_resize reports;
 *   save    — 340 x 138, then what save_resize reports;
 *   passkey — 380 x 180, then what pk_resize reports;
 *   tab     — a browser tab (`width` x `height`).
 * `replies(text)`: the background's reply per message type (arrays are consumed in order).
 */
export const extensionScenarios = [
  // ---------------------------------------------------------------- popup
  ...[
    ["host-unavailable", { kind: "host_unavailable" }],
    ["desktop-unavailable", { kind: "desktop_unavailable" }],
    ["no-vault", { kind: "no_vault" }],
    ["locked", { kind: "locked" }],
    ["disabled", { kind: "disabled" }],
  ].map(([name, state]) => ({
    name: `popup-${name}`,
    page: "popup.html",
    frame: { kind: "popup" },
    replies: () => ({ popup_state: ok(state) }),
  })),
  {
    name: "popup-error",
    page: "popup.html",
    frame: { kind: "popup" },
    replies: (x) => ({ popup_state: err(x.hostUnavailable) }),
  },
  {
    name: "popup-unlocked-matches",
    page: "popup.html",
    frame: { kind: "popup" },
    granted: true,
    replies: () => ({ popup_state: ok({ kind: "unlocked", site: "github.com", matches }) }),
  },
  {
    name: "popup-unlocked-code-and-error",
    page: "popup.html",
    frame: { kind: "popup" },
    granted: true,
    replies: (x) => ({
      popup_state: ok({ kind: "unlocked", site: "github.com", matches }),
      popup_totp: ok({ code: "38149207", secondsRemaining: 25 }),
      popup_fill: err(x.integrationDisabled),
    }),
    async act(page) {
      await page.click("li.item:nth-child(1) .actions span .btn");
      await page.click("li.item:nth-child(2) .actions .btn");
    },
  },
  {
    name: "popup-unlocked-offer",
    page: "popup.html",
    frame: { kind: "popup" },
    granted: false,
    replies: () => ({ popup_state: ok({ kind: "unlocked", site: "accounts.example-with-a-long-hostname.com", matches: matches.slice(0, 1) }) }),
  },
  {
    name: "popup-unlocked-no-matches",
    page: "popup.html",
    frame: { kind: "popup" },
    granted: false,
    replies: () => ({ popup_state: ok({ kind: "unlocked", site: "example.com", matches: [] }) }),
  },
  {
    name: "popup-unlocked-no-page",
    page: "popup.html",
    frame: { kind: "popup" },
    granted: true,
    replies: () => ({ popup_state: ok({ kind: "unlocked", site: null, matches: [] }) }),
  },

  // ---------------------------------------------------------------- inline menu
  ...[260, 360].flatMap((width) => [
    {
      name: `menu-logins-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 3 },
      replies: () => ({ menu_state: ok(menuView({ items: menuItems })) }),
    },
    {
      name: `menu-passkeys-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 3 },
      replies: () => ({
        menu_state: ok(
          menuView({
            passkeys: [{ itemId: ID1, credentialId: CRED1, title: "GitHub", userName: "octocat" }],
            hint: { kind: "use_passkey" },
          }),
        ),
      }),
    },
    {
      name: `menu-add-passkey-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 2 },
      replies: () => ({ menu_state: ok(menuView({ items: menuItems.slice(0, 1), hint: { kind: "add_passkey", name: "GitHub" } })) }),
    },
    {
      name: `menu-otp-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 1 },
      replies: () => ({ menu_state: ok(menuView({ kind: "otp", items: menuItems.slice(0, 1) })) }),
    },
    {
      name: `menu-generate-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 1 },
      replies: () => ({ menu_state: ok(menuView({ kind: "new_password", items: [] })) }),
    },
    {
      name: `menu-locked-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 1 },
      replies: () => ({ menu_state: ok({ state: "locked" }) }),
    },
    {
      name: `menu-empty-logins-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 1 },
      replies: () => ({ menu_state: ok(menuView({ items: [] })) }),
    },
    {
      name: `menu-empty-otp-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 1 },
      replies: () => ({ menu_state: ok(menuView({ kind: "otp", items: [] })) }),
    },
    {
      name: `menu-unavailable-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 1 },
      replies: (x) => ({ menu_state: err(x.hostUnavailable) }),
    },
    {
      name: `menu-fill-error-${width}`,
      page: "menu.html",
      frame: { kind: "menu", width, rows: 2 },
      replies: (x) => ({ menu_state: ok(menuView()), menu_pick: err(x.denied) }),
      async act(page) {
        // The click guard arms after the frame has been visible for 400 ms.
        await page.waitForTimeout(600);
        await page.click("button.row");
      },
    },
  ]),

  // ---------------------------------------------------------------- save prompt
  {
    name: "save-add",
    page: "save.html",
    frame: { kind: "save" },
    replies: () => ({ save_state: ok({ action: "add", site: "github.com", username: "octocat@example.com" }) }),
  },
  {
    name: "save-update-long",
    page: "save.html",
    frame: { kind: "save" },
    replies: () => ({
      save_state: ok({
        action: "update",
        site: "accounts.example-with-a-long-hostname.com",
        username: "firstname.lastname.with-a-long-address@corporate-example.com",
      }),
    }),
  },
  {
    name: "save-no-username",
    page: "save.html",
    frame: { kind: "save" },
    replies: () => ({ save_state: ok({ action: "add", site: "github.com", username: null }) }),
  },
  {
    name: "save-expired",
    page: "save.html",
    frame: { kind: "save" },
    replies: (x) => ({ save_state: err(x.promptExpired) }),
  },
  {
    name: "save-host-unavailable",
    page: "save.html",
    frame: { kind: "save" },
    replies: (x) => ({ save_state: err(x.hostUnavailable) }),
  },
  {
    name: "save-confirm-error",
    page: "save.html",
    frame: { kind: "save" },
    replies: (x) => ({
      save_state: ok({ action: "add", site: "github.com", username: "octocat@example.com" }),
      save_confirm: err(x.integrationDisabled),
    }),
    async act(page) {
      await page.waitForTimeout(600);
      await page.click("#confirm");
    },
  },

  // ---------------------------------------------------------------- passkey card
  {
    name: "passkey-locked",
    page: "passkey.html",
    frame: { kind: "passkey" },
    replies: () => ({ pk_state: ok({ state: "locked", site: "github.com" }) }),
  },
  {
    name: "passkey-chooser",
    page: "passkey.html",
    frame: { kind: "passkey" },
    replies: () => ({
      pk_state: ok({
        state: "chooser",
        site: "github.com",
        passkeys: [
          { itemId: ID1, credentialId: CRED1, title: "GitHub", userName: "octocat" },
          { itemId: ID2, credentialId: CRED2, title: "GitHub — work account for the platform team", userName: "firstname.lastname@corporate-example.com" },
        ],
      }),
    }),
  },
  {
    name: "passkey-create",
    page: "passkey.html",
    frame: { kind: "passkey" },
    replies: () => ({
      pk_state: ok({
        state: "create",
        site: "github.com",
        userName: "octocat",
        candidates: [
          { itemId: ID1, title: "GitHub", username: "octocat" },
          { itemId: ID2, title: "GitHub — work account for the platform team", username: "firstname.lastname@corporate-example.com" },
          { itemId: ID3, title: "GitHub (old)", username: null },
        ],
        upgradeItemId: null,
      }),
    }),
  },
  {
    name: "passkey-upgrade-no-name",
    page: "passkey.html",
    frame: { kind: "passkey" },
    replies: () => ({
      pk_state: ok({ state: "create", site: "github.com", userName: "", candidates: [{ itemId: ID1, title: "GitHub", username: "octocat" }], upgradeItemId: ID1 }),
    }),
  },
  {
    name: "passkey-exists",
    page: "passkey.html",
    frame: { kind: "passkey" },
    replies: () => ({ pk_state: ok({ state: "exists", site: "github.com" }) }),
  },
  {
    name: "passkey-saved",
    page: "passkey.html",
    frame: { kind: "passkey" },
    replies: () => ({ pk_state: ok({ state: "saved", site: "github.com" }) }),
  },
  {
    name: "passkey-error",
    page: "passkey.html",
    frame: { kind: "passkey" },
    replies: (x) => ({ pk_state: err(x.hostUnavailable) }),
  },

  // ---------------------------------------------------------------- options
  ...[
    ["off", 0],
    ["on", 2],
    ["https-only", 1],
  ].flatMap(([name, granted]) =>
    [
      [1000, 760],
      [560, 900],
    ].map(([width, height]) => ({
      name: `options-${name}-${width}`,
      page: "options.html",
      frame: { kind: "tab", width, height },
      granted,
      replies: () => ({}),
    })),
  ),
];

/**
 * The chrome.* stub, installed before any page script runs. Serialized into
 * the page: no closures. `setup`: { locale, replies, granted }.
 */
export function chromeStub(setup) {
  const replies = setup.replies;
  const sent = [];
  window.__hk = { sent, sizes: {} };
  const reply = (msg) => {
    sent.push(msg);
    const type = msg && msg.type;
    if (type === "menu_resize" || type === "save_resize" || type === "pk_resize") {
      window.__hk.sizes[type] = msg.height;
      return { ok: true, value: null };
    }
    const r = replies[type];
    if (Array.isArray(r)) return r.length > 1 ? r.shift() : r[0];
    return r === undefined ? { ok: true, value: null } : r;
  };
  const noopEvent = { addListener() {}, removeListener() {}, hasListener: () => false };
  // Which optional host origins count as granted: all, some or none.
  const grantedCount = setup.granted === true ? 2 : setup.granted === false ? 0 : setup.granted ?? 0;
  const origins = ["https://*/*", "http://*/*"];
  window.chrome = {
    i18n: { getUILanguage: () => setup.locale, getMessage: () => "" },
    runtime: {
      id: "havenkeysuicheck",
      sendMessage: async (msg) => JSON.parse(JSON.stringify(reply(msg))),
      getURL: (p) => new URL(p, location.href).href,
      openOptionsPage: async () => undefined,
      onMessage: noopEvent,
      connect: () => ({ postMessage() {}, onMessage: noopEvent, onDisconnect: noopEvent, disconnect() {} }),
    },
    permissions: {
      contains: async ({ origins: asked }) => asked.every((o) => origins.indexOf(o) > -1 && origins.indexOf(o) < grantedCount),
      getAll: async () => ({ origins: origins.slice(0, grantedCount), permissions: [] }),
      request: async () => false,
      remove: async () => false,
      onAdded: noopEvent,
      onRemoved: noopEvent,
    },
    storage: { local: { get: async () => ({}), set: async () => undefined }, onChanged: noopEvent },
  };
}

/** Load one scenario, size it like the browser would, run its actions. */
export async function renderExtension(page, baseUrl, scenario, locale) {
  const text = TEXT[locale];
  const f = scenario.frame;
  const hash = f.kind === "popup" || f.kind === "tab" ? "" : `#${TOKEN}`;
  let width;
  let height;
  if (f.kind === "popup") [width, height] = [320, 600];
  else if (f.kind === "menu") [width, height] = [f.width, MENU_HEADER + f.rows * MENU_ROW + MENU_PADDING];
  else if (f.kind === "save") [width, height] = [SAVE.width, SAVE.height];
  else if (f.kind === "passkey") [width, height] = [PASSKEY.width, PASSKEY.height];
  else [width, height] = [f.width, f.height];
  await page.setViewportSize({ width, height });
  await page.addInitScript(chromeStub, { locale, replies: scenario.replies(text), granted: scenario.granted ?? 0 });
  await page.goto(`${baseUrl}/ext/${scenario.page}${hash}`);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(150);
  if (scenario.act) {
    await scenario.act(page);
    await page.waitForTimeout(250);
  }

  // Follow the size the page reports, as the content script / bridge would.
  const key = { menu: "menu_resize", save: "save_resize", passkey: "pk_resize" }[f.kind];
  if (key) {
    for (let i = 0; i < 4; i++) {
      await page.waitForTimeout(120);
      const reported = await page.evaluate((k) => window.__hk.sizes[k] ?? null, key);
      if (reported === null) break;
      let next = reported;
      if (f.kind === "passkey") next = Math.min(reported, 900);
      if (next === height) break;
      height = next;
      await page.setViewportSize({ width, height });
    }
  } else if (f.kind === "popup") {
    // A popup is as tall as its content, up to the browser's 600px cap.
    const h = await page.evaluate(() => Math.ceil(document.documentElement.getBoundingClientRect().height));
    height = Math.min(600, Math.max(1, h));
    await page.setViewportSize({ width, height });
  }
  await page.waitForTimeout(100);
  return { width, height };
}

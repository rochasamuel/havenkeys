// The desktop UI (the Vite build) under a stubbed Tauri IPC: every screen
// and the states worth seeing, driven by clicks where a state needs one.

const DAY = 86_400_000;
const NOW = Date.UTC(2026, 8, 20, 14, 30);

const items = [
  {
    id: "7c9e6679-7425-40de-944b-e07fc1f90ae7",
    itemType: "login",
    title: "GitHub",
    username: "octocat@example.com",
    urls: [
      { url: "https://github.com", matchType: "domain" },
      { url: "https://accounts.example-with-a-rather-long-hostname.com/signin/v2/identifier?flow=login", matchType: "exact" },
    ],
    hasPassword: true,
    hasTotp: true,
    hasNotes: true,
    hasPasskey: true,
    autoSignIn: true,
    createdAt: NOW - 400 * DAY,
    updatedAt: NOW - 3 * DAY,
  },
  {
    id: "11111111-2222-4333-8444-555555555555",
    itemType: "login",
    title: "Company single sign-on — platform infrastructure team account",
    username: "firstname.lastname.with-a-long-address@corporate-example.com",
    urls: [{ url: "https://sso.corporate-example.com", matchType: "origin" }],
    hasPassword: true,
    hasTotp: false,
    hasNotes: false,
    hasPasskey: false,
    autoSignIn: false,
    createdAt: NOW - 90 * DAY,
    updatedAt: NOW - 90 * DAY,
  },
  {
    id: "22222222-3333-4444-8555-666666666666",
    itemType: "secure_note",
    title: "Wi-Fi at home",
    username: null,
    urls: [],
    hasPassword: false,
    hasTotp: false,
    hasNotes: false,
    hasPasskey: false,
    autoSignIn: false,
    createdAt: NOW - 30 * DAY,
    updatedAt: NOW - 2 * DAY,
  },
  {
    id: "33333333-4444-4555-8666-777777777777",
    itemType: "login",
    title: "Bank",
    username: null,
    urls: [{ url: "https://bank.example", matchType: "exact" }],
    hasPassword: true,
    hasTotp: true,
    hasNotes: false,
    hasPasskey: false,
    autoSignIn: true,
    createdAt: NOW - 10 * DAY,
    updatedAt: NOW - 10 * DAY,
  },
  {
    id: "44444444-5555-4666-8777-888888888888",
    itemType: "card",
    title: "Mastercard",
    username: null,
    urls: [],
    hasPassword: false,
    hasTotp: false,
    hasNotes: true,
    hasPasskey: false,
    autoSignIn: true,
    card: { brand: "mastercard", last4: "7609", expiry: "2033-11" },
    createdAt: NOW - 20 * DAY,
    updatedAt: NOW - 20 * DAY,
  },
  {
    id: "00f03a59-33cc-8082-a649-ca545b3b372d",
    itemType: "identity",
    title: "Samuel Rocha",
    username: "samuel.rocha@example.com",
    urls: [],
    hasPassword: false,
    hasTotp: false,
    hasNotes: true,
    hasPasskey: false,
    autoSignIn: true,
    createdAt: NOW - 5 * DAY,
    updatedAt: NOW - 1 * DAY,
  },
];

const IDENTITY_ID = "00f03a59-33cc-8082-a649-ca545b3b372d";

const identity = {
  fields: {
    firstName: "Samuel",
    lastName: "Rocha",
    gender: "Masculino",
    birthDate: "2000-04-20",
    occupation: "Desenvolvedor de Software",
    cpf: "123.456.789-00",
    rg: "1.234.567 SSP/DF",
    email: "samuel.rocha@example.com",
    mobilePhone: "+55 (61) 99999-0000",
    street: "Quadra 02 Conjunto 01 (Setor Especial) with a long street name",
    number: "10",
    neighborhood: "Estrutural",
    city: "Brasília",
    state: "DF",
    postalCode: "71266-105",
    country: "Brasil",
    username: "samuelrocha",
    website: "https://example.com/",
    custom: [
      { label: "Blood type", value: "O+", hidden: false },
      { label: "Library card PIN", value: "4321", hidden: true },
    ],
    notes: "Allergic to penicillin.",
  },
  address: "Quadra 02 Conjunto 01 (Setor Especial) with a long street name, 10\nEstrutural\nBrasília – DF\nCEP 71266-105\nBrasil",
};

/** A login's custom fields (login_fields): every type, a titled and an untitled section. */
const loginFields = [
  {
    id: "c1000000-0000-4000-8000-000000000001",
    title: null,
    fields: [
      { id: "f1000000-0000-4000-8000-000000000001", type: "text", label: "Recovery codes", value: "4f9a-22c1 7d0e-91b3\n0c5a-6e2f 33d8-a1c7" },
      { id: "f1000000-0000-4000-8000-000000000002", type: "password", label: "Deploy key passphrase", hasValue: true },
      { id: "f1000000-0000-4000-8000-000000000003", type: "otp", label: "Backup authenticator", hasOtp: true },
    ],
  },
  {
    id: "c1000000-0000-4000-8000-000000000002",
    title: "Billing contact with a rather long section title",
    fields: [
      { id: "f1000000-0000-4000-8000-000000000004", type: "email", label: "Billing email", value: "billing.department.long-address@corporate-example.com" },
      { id: "f1000000-0000-4000-8000-000000000005", type: "phone", label: "Phone", value: "+55 (61) 99999-0000" },
      { id: "f1000000-0000-4000-8000-000000000006", type: "url", label: "Billing portal", value: "https://billing.example-with-a-rather-long-hostname.com/account/settings" },
      { id: "f1000000-0000-4000-8000-000000000007", type: "date", label: "Renewal", value: "2027-03-14" },
      {
        id: "f1000000-0000-4000-8000-000000000008",
        type: "address",
        label: "Invoice address",
        parts: { street: "Quadra 02 Conjunto 01 (Setor Especial)", number: "10", neighborhood: "Estrutural", city: "Brasília", state: "DF", postalCode: "71266-105", country: "Brasil" },
        formatted: "Quadra 02 Conjunto 01 (Setor Especial), 10\nEstrutural\nBrasília – DF\nCEP 71266-105\nBrasil",
      },
      { id: "f1000000-0000-4000-8000-000000000009", type: "password", label: "Empty secret", hasValue: false },
    ],
  },
];

const settings = {
  autoLockMinutes: 15,
  clipboardClearSeconds: 30,
  theme: "dark",
  browserIntegration: true,
  autoPasskeyUpgrade: false,
  autoSignIn: false,
};

const kit = {
  secretKey: "H1-ABCDEF-GHJKMN-PQRSTV-WXYZ23",
  vaultId: "5f0e8a3c-2b1d-4e6f-9a7b-8c9d0e1f2a3b",
  accountId: "9b8a7c6d-5e4f-4a3b-8c2d-1e0f9a8b7c6d",
  email: "firstname.lastname@example.com",
  serverUrl: "https://vault.example.com",
  createdAt: NOW - 400 * DAY,
  qrSize: 25,
  qrModules: Array.from({ length: 625 }, (_, i) => ((i * 7919) % 13) < 6),
};

/** Canned answers per command. Values that are `{ reject }` reject with that error. */
function baseResponses() {
  return {
    vault_status: { state: "unlocked", vaultExists: true, damagedItems: 0, unreadableItems: 0 },
    device_status: { keyScheme: "account_bound", needsSecretKey: false, online: true, secretKeyStorage: "keychain" },
    list_items: items,
    identity_item_id: IDENTITY_ID,
    reveal_identity: identity,
    copy_identity_field: { clearAfterSeconds: 30 },
    reveal_card: {
      cardholderName: "Samuel S Rocha",
      brand: null,
      expiry: "2033-11",
      notes: "Nubank — limit raised in March.",
      hasNumber: true,
      hasVerificationNumber: true,
    },
    reveal_card_field: "5200828282827609",
    copy_card_field: { clearAfterSeconds: 30 },
    check_card_number: { brand: "mastercard", checkDigitOk: false },
    login_fields: [],
    reveal_login_field: "a deploy key passphrase that is rather long",
    login_field_totp: { code: "77104523", period: 30, secondsRemaining: 4 },
    copy_login_field: { clearAfterSeconds: 30 },
    open_login_field_url: null,
    get_item: items[0],
    reveal_secret: "correct horse battery staple — a long passphrase note\nSecond line of the note.",
    password_history: [NOW - 40 * DAY, NOW - 200 * DAY],
    list_passkeys: [
      { credentialId: "AQEBAQEBAQEBAQEBAQEBAQ", rpId: "github.com", userName: "octocat", displayName: "The Octocat", createdAt: NOW - 20 * DAY },
    ],
    get_totp_code: { code: "381492", period: 30, secondsRemaining: 21 },
    copy_secret: { clearAfterSeconds: 30 },
    generate_password: { password: "v7#Qm2!rX9$kLp4@Wz8^Nt", entropyBits: 142 },
    copy_generated_password: { clearAfterSeconds: 30 },
    get_settings: settings,
    update_settings: settings,
    launch_at_login: false,
    set_launch_at_login: false,
    set_ui_language: null,
    record_activity: null,
    account_status: {
      email: "firstname.lastname@example.com",
      serverUrl: "https://vault.example-with-a-long-hostname.com",
      accountId: kit.accountId,
      online: true,
      lastSyncedAt: NOW - 5 * 60_000,
    },
    list_devices: [
      { id: "d1", name: "Work laptop (Ubuntu 24.04)", createdAt: "2026-01-02T10:00:00Z", lastSeenAt: new Date(Date.now() - 60_000).toISOString(), current: true },
      { id: "d2", name: "Home desktop", createdAt: "2026-02-02T10:00:00Z", lastSeenAt: new Date(Date.now() - 3 * DAY).toISOString(), current: false },
    ],
    get_emergency_kit: kit,
    reveal_account_secret_key: "H1-A3F9KQ-7XR2PL-M8WD4T-JC6VNB-2HYE5S",
    copy_account_field: { clearAfterSeconds: 30 },
    import_1pux: {
      fileName: "1PasswordExport-ABCDEFGHIJKLMNOPQRSTUVWX-20260920-143000.1pux",
      report: {
        imported: 212,
        logins: 190,
        secureNotes: 22,
        cards: 6,
        convertedToNotes: 3,
        skippedDuplicates: 4,
        skippedArchived: 2,
        failed: 1,
        attachmentsSkipped: 5,
        passwordHistorySkipped: 7,
        urlsMovedToNotes: 2,
      },
    },
    delete_import_file: null,
    lock_vault: null,
  };
}

const reject = (code, message = "Error from the core.") => ({ reject: { code, message } });

// Selectors: structure, not text, so one script drives both languages.
const nav = (n) => `.nav .nav-item:nth-of-type(${n})`;
const SETTINGS = ".nav > button.nav-item:last-of-type";
const GENERATOR = ".nav > button.nav-item:nth-last-of-type(2)";
const firstItem = ".list-items li:nth-child(1) .list-item";

async function scrollTool(page, where) {
  await page.evaluate((w) => {
    const el = document.querySelector(".tool") ?? document.querySelector(".detail");
    const target = typeof w === "string" ? document.querySelector(w) : null;
    if (target) target.scrollIntoView({ block: "start" });
    else if (el) el.scrollTop = w;
  }, where);
}

/**
 * `respond`: overrides on the base responses. `act(page)`: clicks to reach
 * the state. `shots`: extra screenshots after scrolling (selector to bring to
 * the top of the scroll area).
 */
export const desktopScenarios = [
  // ---------------------------------------------------------------- first run
  { name: "welcome-invite", respond: { vault_status: { state: "locked", vaultExists: false, damagedItems: 0, unreadableItems: 0 } } },
  {
    name: "welcome-invite-errors",
    respond: {
      vault_status: { state: "locked", vaultExists: false, damagedItems: 0, unreadableItems: 0 },
      activate_account: reject("invalid_input", "Invalid input: invite."),
    },
    async act(page) {
      await page.fill(".welcome-form textarea", "HKINV1-EXAMPLE");
      const pw = page.locator(".welcome-form input[type=password]");
      await pw.nth(0).fill("short");
      await pw.nth(1).fill("different");
    },
  },
  {
    name: "welcome-invite-server-error",
    respond: {
      vault_status: { state: "locked", vaultExists: false, damagedItems: 0, unreadableItems: 0 },
      activate_account: reject("offline"),
    },
    async act(page) {
      await page.fill(".welcome-form textarea", "HKINV1-EXAMPLE");
      const pw = page.locator(".welcome-form input[type=password]");
      await pw.nth(0).fill("a long enough password");
      await pw.nth(1).fill("a long enough password");
      await page.click(".welcome-form button[type=submit]");
    },
  },
  {
    name: "welcome-signin",
    respond: { vault_status: { state: "locked", vaultExists: false, damagedItems: 0, unreadableItems: 0 }, sign_in: reject("sign_in_failed") },
    async act(page) {
      await page.click(".segmented button:nth-child(2)");
      await page.fill(".welcome-form input >> nth=0", "https://vault.example.com");
      await page.fill(".welcome-form input >> nth=1", "me@example.com");
      await page.fill(".welcome-form input >> nth=2", "password");
      await page.click(".welcome-form button[type=submit]");
    },
  },
  {
    name: "welcome-removed-warning",
    respond: { vault_status: { state: "unlocked", vaultExists: true, damagedItems: 0, unreadableItems: 0 } },
    async act(page) {
      await page.evaluate(() => {
        window.__hkRespond.vault_status = { state: "locked", vaultExists: false, damagedItems: 0, unreadableItems: 0 };
        window.__hkEmit("vault://removed", { keychainWarning: "x" });
      });
    },
  },
  {
    name: "kit-after-activation",
    respond: { vault_status: { state: "locked", vaultExists: false, damagedItems: 0, unreadableItems: 0 } },
    async act(page) {
      await page.evaluate(() => {
        window.__hkRespond.activate_account = { state: "unlocked", vaultExists: true, damagedItems: 0, unreadableItems: 0 };
      });
      await page.fill(".welcome-form textarea", "HKINV1-EXAMPLE");
      const pw = page.locator(".welcome-form input[type=password]");
      await pw.nth(0).fill("a long enough password");
      await pw.nth(1).fill("a long enough password");
      await page.click(".welcome-form button[type=submit]");
    },
    shots: [".kit-actions"],
  },
  { name: "fatal", respond: { vault_status: reject("vault_unreadable", "The vault file in /home/user/.local/share/app.havenkeys could not be read.") } },

  // ---------------------------------------------------------------- unlock
  { name: "unlock", respond: { vault_status: { state: "locked", vaultExists: true, damagedItems: 0, unreadableItems: 0 } } },
  {
    name: "unlock-wrong-password",
    respond: { vault_status: { state: "locked", vaultExists: true, damagedItems: 0, unreadableItems: 0 }, unlock_vault: reject("unlock_failed") },
    async act(page) {
      await page.fill(".unlock-field input", "nope");
      await page.click(".unlock-go");
      await page.waitForTimeout(500);
    },
  },
  {
    name: "unlock-keychain",
    respond: { vault_status: { state: "locked", vaultExists: true, damagedItems: 0, unreadableItems: 0 }, unlock_vault: reject("keychain_unavailable") },
    async act(page) {
      await page.fill(".unlock-field input", "nope");
      await page.click(".unlock-go");
      await page.waitForTimeout(500);
    },
  },
  {
    name: "unlock-secret-key",
    respond: {
      vault_status: { state: "locked", vaultExists: true, damagedItems: 0, unreadableItems: 0 },
      device_status: { keyScheme: "account_bound", needsSecretKey: true, online: false, secretKeyStorage: "none" },
    },
  },
  {
    name: "unlock-after-idle",
    respond: {},
    async act(page) {
      await page.evaluate(() => window.__hkEmit("vault://locked", { reason: "screen_lock" }));
    },
  },

  // ---------------------------------------------------------------- vault
  { name: "vault-empty", respond: { list_items: [] } },
  { name: "vault-list", respond: {} },
  {
    name: "vault-banners",
    respond: {
      vault_status: { state: "unlocked", vaultExists: true, damagedItems: 2, unreadableItems: 3 },
      device_status: { keyScheme: "account_bound", needsSecretKey: false, online: false, secretKeyStorage: "file" },
    },
  },
  {
    name: "vault-signed-out",
    respond: {},
    async act(page) {
      await page.evaluate(() => window.__hkEmit("vault://signed-out", null));
    },
  },
  {
    name: "vault-new-menu",
    respond: {},
    async act(page) {
      await page.click(".new-menu .icon-btn");
    },
  },
  {
    name: "vault-logins-section",
    respond: {},
    async act(page) {
      await page.click(nav(2));
    },
  },
  {
    name: "vault-search-none",
    respond: {},
    async act(page) {
      await page.evaluate(() => (window.__hkRespond.list_items = []));
      await page.fill(".search input", "nothing matches this");
      await page.waitForTimeout(300);
    },
  },

  // ---------------------------------------------------------------- item detail
  {
    name: "detail-login",
    respond: {},
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
    },
    shots: [".group-history"],
  },
  {
    name: "detail-login-revealed",
    respond: {},
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
      // Password eye, notes eye, history, passkey delete, item delete.
      await page.click(".item .group:first-of-type .icon-btn >> nth=0");
      await page.click(".group-history .row-button");
      await page.waitForTimeout(200);
    },
    shots: [".group-history", ".item-foot"],
  },
  {
    name: "detail-login-confirms",
    respond: {},
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
      await page.click(".history-row .icon-btn");
      await page.click(".item-foot .btn");
      await page.waitForTimeout(150);
    },
    shots: [".history-list", ".item-foot"],
  },
  {
    name: "detail-custom-fields",
    respond: { login_fields: loginFields },
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
      // The custom Password field's eye.
      await page.click(".item .row:has(.secret) >> nth=1 >> .icon-btn >> nth=0");
      await page.waitForTimeout(150);
    },
    shots: [".item .group:has(.totp-code) >> nth=1", ".group-history"],
  },
  {
    name: "detail-custom-fields-failed",
    respond: { login_fields: reject("internal") },
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
    },
    shots: [".group-note"],
  },
  {
    name: "detail-long-title",
    respond: {},
    async act(page) {
      await page.click(".list-items li:nth-child(2) .list-item");
      await page.waitForTimeout(200);
      await page.click(".item-foot .btn");
    },
  },
  {
    name: "detail-note",
    respond: {},
    async act(page) {
      await page.click(".list-items li:nth-child(3) .list-item");
      await page.waitForTimeout(200);
    },
  },
  {
    name: "identity",
    respond: {},
    act: async (page) => {
      await page.click(nav(5));
    },
  },
  {
    name: "identity-revealed",
    respond: {},
    act: async (page) => {
      await page.click(nav(5));
      await page.click(".item section:nth-of-type(2) .icon-btn >> nth=0");
      await page.click(".item .copy-btn >> nth=0");
    },
  },
  {
    name: "identity-empty",
    respond: {
      list_items: items.map((i) => (i.itemType === "identity" ? { ...i, title: "", username: null, hasNotes: false } : i)),
      reveal_identity: { fields: {}, address: null },
    },
    act: async (page) => {
      await page.click(nav(5));
    },
  },
  {
    name: "identity-editor",
    respond: {},
    act: async (page) => {
      await page.click(nav(5));
      await page.click(".item-head-actions .btn");
    },
  },
  {
    name: "identity-not-created",
    respond: { list_items: items.filter((i) => i.itemType !== "identity") },
  },
  {
    name: "cards",
    respond: {},
    act: async (page) => {
      await page.click(nav(4));
    },
  },
  {
    name: "card-detail",
    respond: {},
    act: async (page) => {
      await page.click(".list-item:has(.avatar-card)");
    },
  },
  {
    name: "card-revealed",
    respond: {},
    act: async (page) => {
      await page.click(".list-item:has(.avatar-card)");
      await page.click(".item .icon-btn:not(.copy-btn) >> nth=0");
    },
  },
  {
    name: "card-expired",
    respond: {
      list_items: items.map((i) => (i.itemType === "card" ? { ...i, card: { ...i.card, expiry: "2020-01" } } : i)),
      reveal_card: { cardholderName: null, brand: null, expiry: "2020-01", notes: null, hasNumber: true, hasVerificationNumber: false },
    },
    act: async (page) => {
      await page.click(".list-item:has(.avatar-card)");
    },
  },
  {
    name: "card-editor",
    respond: {},
    act: async (page) => {
      await page.click(".list-item:has(.avatar-card)");
      await page.click(".item-head-actions .btn");
      await page.fill(".card-number-input input", "5200 8282 8282 7600");
      await page.waitForTimeout(400);
    },
  },
  {
    name: "account-item",
    respond: {},
    act: async (page) => {
      await page.click(".list-pinned .list-item");
    },
  },
  {
    name: "account-item-revealed",
    respond: {},
    act: async (page) => {
      await page.click(".list-pinned .list-item");
      await page.click(".item .icon-btn[aria-label*='Secret Key'] >> nth=0");
      await page.click(".item .copy-btn >> nth=0");
    },
  },
  {
    name: "account-item-no-key",
    respond: {
      device_status: { keyScheme: "account_bound", needsSecretKey: true, online: true, secretKeyStorage: "none" },
    },
    act: async (page) => {
      await page.click(".list-pinned .list-item");
    },
  },
  {
    name: "detail-toast",
    respond: {},
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
      await page.click(".item .group:first-of-type .field:first-child .icon-btn, .item .group:first-of-type button[aria-label] >> nth=0");
      await page.waitForTimeout(200);
    },
  },

  // ---------------------------------------------------------------- editor
  {
    name: "editor-login",
    respond: {},
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
      await page.click(".item-head-actions .btn");
      await page.waitForTimeout(200);
    },
    shots: [".editor .group-title >> nth=0", ".edit-area"],
  },
  {
    name: "editor-login-changing",
    respond: {},
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
      await page.click(".item-head-actions .btn");
      await page.waitForTimeout(200);
      // Password "Change", then the codes' "Remove" (second secret row).
      await page.click(".edit-secret .btn >> nth=0");
      await page.click(".edit-secret .btn-quiet-danger >> nth=0");
    },
    shots: [".edit-area"],
  },
  {
    name: "editor-new-login",
    respond: {},
    async act(page) {
      await page.click(".new-menu .icon-btn");
      await page.click(".menu button >> nth=0");
      await page.waitForTimeout(200);
    },
    shots: [".edit-area"],
  },
  {
    name: "editor-custom-fields",
    respond: { login_fields: loginFields },
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
      await page.click(".item-head-actions .btn");
      await page.waitForTimeout(200);
      // Replace the kept Password field; ask to remove the second section.
      await page.click(".cf-row .edit-secret .btn >> nth=0");
      await page.click(".cf-section >> nth=1 >> .cf-section-head .icon-btn >> nth=-1");
    },
    shots: [".cf-section >> nth=0", ".cf-section >> nth=1", ".cf-add-group"],
  },
  {
    name: "editor-custom-fields-new",
    respond: {},
    async act(page) {
      await page.click(".new-menu .icon-btn");
      await page.click(".menu button >> nth=0");
      await page.waitForTimeout(200);
      // One of each type, then an empty section, then the menu open again.
      for (let i = 0; i < 8; i++) {
        await page.click(".cf-add .add-row");
        await page.click(`.cf-menu button >> nth=${i}`);
      }
      await page.click(".cf-add-group > .add-row");
      await page.click(".cf-add .add-row");
    },
    shots: [".cf-section", ".cf-row:has(.cf-address)", ".cf-add-group"],
  },
  {
    name: "editor-new-note",
    respond: {},
    async act(page) {
      await page.click(".new-menu .icon-btn");
      await page.click(".menu button >> nth=1");
      await page.waitForTimeout(200);
    },
  },
  {
    name: "editor-save-error",
    respond: { update_item: reject("item_changed_elsewhere") },
    async act(page) {
      await page.click(firstItem);
      await page.waitForTimeout(200);
      await page.click(".item-head-actions .btn");
      await page.waitForTimeout(200);
      await page.fill(".edit-input >> nth=0", "GitHub personal");
      await page.click(".editor-actions .btn-primary");
      await page.waitForTimeout(300);
    },
    shots: [".form-error"],
  },

  // ---------------------------------------------------------------- tools
  {
    name: "generator",
    respond: {},
    async act(page) {
      await page.click(GENERATOR);
      await page.waitForTimeout(200);
    },
    shots: [".tool .group-title >> nth=1"],
  },
  {
    name: "settings",
    respond: { launch_at_login: true },
    async act(page) {
      await page.click(SETTINGS);
      await page.waitForTimeout(300);
    },
    // Walk down the page, a section at a time.
    shots: ["settings-block:2", "settings-block:3", "settings-block:4", "settings-block:5", "settings-block:6", "settings-block:7", "settings-block:8"],
  },
  {
    name: "settings-account-states",
    respond: {},
    async act(page) {
      await page.click(SETTINGS);
      await page.waitForTimeout(300);
      // Device revoke confirm, kit shown, import result with confirm, password mismatch.
      await page.click(".device-row:nth-child(2) .btn");
      await page.click(".kit-teaser .btn");
      await page.waitForTimeout(150);
      const blocks = page.locator(".settings-block");
      const count = await blocks.count();
      for (let i = 0; i < count; i++) {
        const b = blocks.nth(i);
        const importBtn = b.locator(".group-note-top + .group-actions .btn");
        if ((await importBtn.count()) > 0 && (await b.locator(".danger-zone").count()) === 0) {
          await importBtn.first().click();
          await page.waitForTimeout(150);
          await b.locator(".import-result .btn").first().click();
        }
      }
      const pw = page.locator("form.settings-block input[type=password]");
      await pw.nth(0).fill("old");
      await pw.nth(1).fill("new password");
      await pw.nth(2).fill("different");
      await page.fill(".danger-zone input", "x");
    },
    shots: [".device-list", ".kit", ".kit-actions", ".import-result", "form.settings-block", ".danger-zone"],
  },
  {
    name: "settings-offline",
    respond: { device_status: { keyScheme: "account_bound", needsSecretKey: false, online: false, secretKeyStorage: "file" }, account_status: null, list_devices: reject("offline") },
    async act(page) {
      await page.click(SETTINGS);
      await page.waitForTimeout(300);
    },
    shots: ["settings-block:4", "settings-block:5"],
  },
];

/** Bring a scroll target to the top: a selector, or "settings-block:N" (1-based). */
export async function scrollTo(page, target) {
  const m = /^settings-block:(\d+)$/.exec(target);
  if (m) {
    await page.evaluate((n) => document.querySelectorAll(".settings-block")[n - 1]?.scrollIntoView({ block: "start" }), Number(m[1]));
  } else {
    await page.locator(target).first().evaluate((el) => el.scrollIntoView({ block: "start" }));
  }
  await page.waitForTimeout(100);
}

/**
 * The Tauri IPC stub (window.__TAURI_INTERNALS__), installed before the app
 * loads. Serialized into the page: no closures. Records event listeners so
 * the harness can emit vault:// events (window.__hkEmit).
 */
export function tauriStub(setup) {
  const respond = setup.respond;
  window.__hkRespond = respond;
  try {
    if (setup.pref) localStorage.setItem("hk-locale", setup.pref);
  } catch {
    // Storage may be unavailable; the app then follows navigator.languages.
  }
  const callbacks = new Map();
  const listeners = [];
  let next = 1;
  window.__hkEmit = (event, payload) => {
    for (const l of listeners) if (l.event === event) callbacks.get(l.handler)?.({ event, id: l.id, payload });
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback(cb) {
      const id = next++;
      callbacks.set(id, cb);
      return id;
    },
    unregisterCallback(id) {
      callbacks.delete(id);
    },
    convertFileSrc: (p) => p,
    async invoke(cmd, args) {
      if (cmd === "plugin:event|listen") {
        const id = next++;
        listeners.push({ event: args.event, handler: args.handler, id });
        return id;
      }
      if (cmd.startsWith("plugin:")) return null;
      const r = window.__hkRespond[cmd];
      if (r && typeof r === "object" && "reject" in r) throw r.reject;
      if (r === undefined) throw { code: "internal", message: `ui-check: no canned answer for ${cmd}` };
      return JSON.parse(JSON.stringify(r));
    },
  };
}

export function desktopSetup(scenario, locale, theme) {
  const respond = { ...baseResponses(), ...scenario.respond };
  if (respond.get_settings && !("reject" in respond.get_settings)) respond.get_settings = { ...respond.get_settings, theme };
  respond.update_settings = respond.get_settings;
  return { respond, pref: locale };
}

export { scrollTool };

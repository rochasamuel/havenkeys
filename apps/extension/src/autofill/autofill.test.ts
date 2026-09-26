// @vitest-environment jsdom
//
// Autofill engine tests on real DOM trees (jsdom). jsdom has no layout, so
// visibility is decided by a test `Env`: an element is visible unless it
// or an ancestor is `hidden` or has `display:none`/`visibility:hidden`.

import { beforeEach, describe, expect, it } from "vitest";
import { classifyPasswords, groupIntent, usernameScore, type FieldFeatures } from "./classify";
import { fillLogin, fillNewPassword, fillOtp, markUserEdit, setValue } from "./fill";
import { classifyGroup, fieldsOf, groupFor, MAX_GROUP_INPUTS, type Env } from "./group";
import { findLoginGroup, findOtpGroup, readSubmission } from "./page";
import { hasPhrase, normalize } from "./text";

function visible(el: HTMLElement): boolean {
  for (let n: HTMLElement | null = el; n; n = n.parentElement) {
    if (n.hidden) return false;
    const s = getComputedStyle(n);
    if (s.display === "none" || s.visibility === "hidden") return false;
  }
  return true;
}

const env = (path = "/"): Env => ({ isVisible: visible, path });

function page(html: string): void {
  document.body.innerHTML = html;
}

function $(sel: string): HTMLInputElement {
  const el = document.querySelector<HTMLInputElement>(sel);
  if (!el) throw new Error(`missing ${sel}`);
  return el;
}

function kinds(field: string, path = "/"): Record<string, string> {
  const { group } = groupFor($(field), env(path));
  return Object.fromEntries(group.fields.map((f) => [f.el.name || f.el.id, f.kind]));
}

beforeEach(() => page(""));

describe("text", () => {
  it("normalizes attribute soup into words", () => {
    expect(normalize("loginEmail_Address")).toBe("login email address");
    expect(normalize("Usuário")).toBe("usuario");
    expect(normalize("x".repeat(10_000)).length).toBe(200);
  });
  it("matches whole words only", () => {
    expect(hasPhrase("your user name", "user name")).toBe(true);
    expect(hasPhrase("username", "user")).toBe(false);
    expect(hasPhrase("", "user")).toBe(false);
  });
});

describe("field classification", () => {
  const f = (over: Partial<FieldFeatures>): FieldFeatures => ({
    type: "text",
    autocomplete: [],
    attrs: "",
    text: "",
    maxLength: -1,
    inputMode: "",
    ...over,
  });
  const ctx = { hasPassword: true, lastBeforePassword: false };

  it("scores username signals independently", () => {
    expect(usernameScore(f({ autocomplete: ["username"] }), ctx)).toBeGreaterThanOrEqual(100);
    expect(usernameScore(f({ type: "email" }), ctx)).toBeGreaterThanOrEqual(80);
    expect(usernameScore(f({ attrs: "login" }), ctx)).toBe(50);
    expect(usernameScore(f({ attrs: "q", text: "search" }), ctx)).toBeLessThan(0);
    expect(usernameScore(f({ type: "email", autocomplete: ["cc-number"] }), ctx)).toBe(0);
    expect(usernameScore(f({ autocomplete: ["given-name"], attrs: "login" }), ctx)).toBe(0);
    expect(usernameScore(f({ type: "checkbox", attrs: "username" }), ctx)).toBe(0);
  });

  it("resolves password roles from autocomplete, wording, then position", () => {
    const pw = (over: Partial<FieldFeatures> = {}) => f({ type: "password", ...over });
    expect(classifyPasswords([pw()], "login").map((c) => c.kind)).toEqual(["password"]);
    expect(classifyPasswords([pw()], "signup").map((c) => c.kind)).toEqual(["new-password"]);
    expect(classifyPasswords([pw(), pw()], "unknown").map((c) => c.kind)).toEqual([
      "new-password",
      "confirmation-password",
    ]);
    expect(classifyPasswords([pw(), pw(), pw()], "unknown").map((c) => c.kind)).toEqual([
      "current-password",
      "new-password",
      "confirmation-password",
    ]);
    expect(
      classifyPasswords([pw({ autocomplete: ["new-password"] }), pw({ autocomplete: ["new-password"] })], "unknown").map(
        (c) => c.kind,
      ),
    ).toEqual(["new-password", "confirmation-password"]);
    expect(
      classifyPasswords([pw({ text: "current password" }), pw({ text: "new password" })], "unknown").map((c) => c.kind),
    ).toEqual(["current-password", "new-password"]);
    expect(classifyPasswords([pw({ autocomplete: ["current-password"] })], "signup")[0]).toEqual({
      kind: "current-password",
      confidence: 1,
    });
  });

  it("reads the form's intent, weighting the submit button and headings", () => {
    expect(groupIntent("sign in", "no account sign up")).toBe("login");
    expect(groupIntent("create account", "already have an account log in")).toBe("signup");
    expect(groupIntent("change password")).toBe("change");
    expect(groupIntent("")).toBe("unknown");
  });
});

describe("login groups", () => {
  it("normal username + password form", () => {
    page(`<form><h1>Sign in</h1>
      <input name="login" type="text"><input name="password" type="password">
      <button type="submit">Sign in</button></form>`);
    expect(kinds('[name="login"]')).toEqual({ login: "username", password: "password" });
  });

  it("email login with no helpful names", () => {
    page(`<form><input id="a" type="email"><input id="b" type="password"><button>Go</button></form>`);
    expect(kinds("#b")).toEqual({ a: "username", b: "password" });
  });

  it("uses labels and aria-labelledby", () => {
    page(`<div><label for="f1">Nome de usuário</label><input id="f1">
      <span id="l2">Senha</span><input id="f2" type="password" aria-labelledby="l2"></div>`);
    expect(kinds("#f1")).toEqual({ f1: "username", f2: "password" });
  });

  it("password-only step (username entered on a previous page)", () => {
    page(`<form><input type="hidden" name="user" value="octo"><input name="pw" type="password"></form>`);
    const { group, kind } = groupFor($('[name="pw"]'), env());
    expect(kind).toBe("password");
    expect(fieldsOf(group, "username")).toEqual([]);
  });

  it("username-only step needs strong evidence", () => {
    page(`<form><h1>Sign in</h1><input name="identifier" type="email" autocomplete="username"><button type="submit">Next</button></form>`);
    expect(kinds('[name="identifier"]')).toEqual({ identifier: "username" });
    page(`<form><input name="comment" type="text"><button>Send</button></form>`);
    expect(kinds('[name="comment"]')).toEqual({ comment: "unknown" });
  });

  // gov.br marks every field autocomplete="new-password" to keep browsers'
  // own autofill away; the markup below is the real page's.
  it("gov.br: CPF step despite autocomplete=new-password", () => {
    page(`<form id="loginData" action="/login" method="post">
      <p>Digite seu CPF para <strong>criar</strong> ou <strong>acessar</strong> sua conta gov.br</p>
      <label for="cpf">CPF</label>
      <input id="accountId" name="accountId" autocomplete="new-password" tabindex="1" type="tel" inputmode="numeric" value="" placeholder="Digite seu CPF" aria-invalid="false">
      <div class="button-panel" id="login-button-panel">
        <button id="enter-account-id" type="submit" name="operation" value="enter-account-id" class="button-continuar" tabindex="2">Continuar</button>
      </div></form>`);
    expect(kinds("#accountId", "/login")).toEqual({ accountId: "username" });
  });

  it("gov.br: password step marked new-password but worded current", () => {
    page(`<form id="loginData" action="/login" method="post">
      <h3>Digite sua senha</h3><label>CPF</label><h4>000.000.000-00</h4>
      <label for="password">Senha</label>
      <div class="password-eye">
        <input tabindex="1" name="password" id="password" type="password" value="" placeholder="Digite sua senha atual" autocomplete="new-password">
        <span tabindex="2" toggle="#password" class="fa fa-fw fa-eye toggle-password"></span>
      </div>
      <div class="actions"><div class="button-panel">
        <button type="button" value="cancel" class="button-cancel" tabindex="4">Cancelar</button>
        <button id="submit-button" type="submit" name="operation" value="enter-password" class="button-ok" aria-label="Botão Entrar. Aperte a tecla enter para entrar." tabindex="3">Entrar</button>
      </div>
      <button id="password-recovery" type="button" name="operation" value="call-account-recovery" class="button-href-mimic" tabindex="5">Esqueci minha senha</button>
      </div></form>`);
    expect(kinds("#password", "/login")).toEqual({ password: "current-password" });
  });

  it("document-number logins, Brazilian and international", () => {
    const cases: Array<[string, string]> = [
      [`<label for="d">CNPJ</label><input id="d" name="f1">`, "CNPJ"],
      [`<label for="d">RG</label><input id="d" name="f1">`, "RG"],
      [`<input id="d" name="f1" placeholder="Número do documento">`, "documento"],
      [`<label for="d">DNI / NIE</label><input id="d" name="f1">`, "DNI"],
      [`<label for="d">RUT</label><input id="d" name="f1">`, "RUT (Chile)"],
      [`<label for="d">Cédula de ciudadanía</label><input id="d" name="f1">`, "cédula"],
      [`<label for="d">CURP</label><input id="d" name="f1">`, "CURP (Mexico)"],
      [`<label for="d">CUIT / CUIL</label><input id="d" name="f1">`, "CUIT"],
      [`<label for="d">NIF</label><input id="d" name="f1">`, "NIF (Portugal/Spain)"],
      [`<label for="d">Codice fiscale</label><input id="d" name="f1">`, "codice fiscale"],
      [`<label for="d">Passport number</label><input id="d" name="f1">`, "passport"],
      [`<label for="d">Matrícula</label><input id="d" name="f1">`, "matrícula"],
      [`<input id="d" name="numeroDocumento">`, "name attribute"],
    ];
    for (const [field, what] of cases) {
      page(`<form><h1>Entrar</h1>${field}<input name="pw" type="password"><button type="submit">Entrar</button></form>`);
      expect(kinds("#d")["f1"] ?? kinds("#d")["numeroDocumento"], what).toBe("username");
    }
  });

  it("username-only step: a document label is enough on a sign-in form, not elsewhere", () => {
    page(`<form><h1>Entrar</h1><label for="d">CPF</label><input id="d" name="field1" type="tel">
      <button type="submit">Continuar</button></form>`);
    expect(kinds("#d", "/")).toEqual({ field1: "username" });

    // The same box on a checkout form stays unknown: no login intent.
    page(`<form><h1>Finalizar compra</h1><label for="d">CPF</label><input id="d" name="field1" type="tel">
      <button type="submit">Pagar</button></form>`);
    expect(kinds("#d", "/checkout")).toEqual({ field1: "unknown" });

    // A sign-in form's stray box with no username wording stays unknown.
    page(`<form><h1>Sign in</h1><input id="d" name="field1"><button type="submit">Next</button></form>`);
    expect(kinds("#d", "/login")).toEqual({ field1: "unknown" });
  });

  it("a signup password shown as text (show-password toggle) is not a username", () => {
    page(`<form><h1>Sign up</h1><input name="email" type="email">
      <input name="password" type="text" autocomplete="new-password"><input name="password2" type="password" autocomplete="new-password">
      <button type="submit">Sign up</button></form>`);
    expect(kinds('[name="email"]', "/signup")).toMatchObject({ email: "username", password: "unknown" });
  });

  it("does not treat search or newsletter boxes as usernames", () => {
    page(`<form role="search"><input name="q" type="search" placeholder="Search"></form>
      <form><input name="newsletter_email" type="email" placeholder="Subscribe to our newsletter"></form>`);
    expect(kinds('[name="q"]')).toEqual({ q: "unknown" });
    expect(kinds('[name="newsletter_email"]')).toEqual({ newsletter_email: "unknown" });
  });

  it("signup form: new password and confirmation", () => {
    page(`<form><h2>Create your account</h2><input name="email" type="email">
      <input name="pass" type="password"><input name="pass2" type="password">
      <button type="submit">Create account</button><a href="/login">Already have an account? Log in</a></form>`);
    expect(kinds('[name="pass"]', "/signup")).toEqual({ email: "username", pass: "new-password", pass2: "confirmation-password" });
  });

  it("single-password signup is recognized from its intent", () => {
    page(`<form><h1>Sign up</h1><input name="email" type="email"><input name="password" type="password">
      <button type="submit">Sign up</button><a>Log in</a></form>`);
    expect(kinds('[name="password"]', "/register")).toEqual({ email: "username", password: "new-password" });
  });

  it("change-password form with three fields", () => {
    page(`<form><h1>Change password</h1><input name="a" type="password"><input name="b" type="password">
      <input name="c" type="password"><button type="submit">Update</button></form>`);
    expect(kinds('[name="b"]')).toEqual({ a: "current-password", b: "new-password", c: "confirmation-password" });
  });

  it("ignores hidden, disabled and readonly fields", () => {
    page(`<form>
      <input name="trap_user" type="text" style="display:none">
      <input name="trap_pw" type="password" hidden>
      <div style="visibility:hidden"><input name="trap2" type="password"></div>
      <input name="off" type="text" disabled>
      <input name="ro" type="password" readonly>
      <input name="user" type="email"><input name="pw" type="password"></form>`);
    const { group } = groupFor($('[name="pw"]'), env());
    expect(group.fields.map((f) => f.el.name)).toEqual(["user", "pw"]);
  });

  it("OTP: single field and split boxes, not postal codes", () => {
    page(`<form><input name="code" autocomplete="one-time-code" inputmode="numeric" maxlength="6"></form>`);
    expect(kinds('[name="code"]')).toEqual({ code: "otp" });
    page(`<form><label for="z">ZIP code</label><input id="z" maxlength="5" inputmode="numeric"></form>`);
    expect(kinds("#z")).toEqual({ z: "unknown" });
    page(`<div>${[1, 2, 3, 4, 5, 6].map((i) => `<input name="d${i}" maxlength="1">`).join("")}</div>`);
    const { group } = groupFor($('[name="d3"]'), env());
    expect(fieldsOf(group, "otp")).toHaveLength(6);
  });

  it("keeps multiple forms apart", () => {
    page(`<form id="signin"><input name="u1" type="email"><input name="p1" type="password"><button type="submit">Log in</button></form>
      <form id="signup"><h2>Sign up</h2><input name="u2" type="email"><input name="p2" type="password"><input name="p3" type="password"></form>`);
    const { group } = groupFor($('[name="p1"]'), env());
    expect(group.fields.map((f) => f.el.name)).toEqual(["u1", "p1"]);
  });

  it("groups fields without a <form> (SPA markup)", () => {
    page(`<div id="app"><div class="card"><div><input placeholder="Email"></div>
      <div><input type="password" placeholder="Password"></div><div role="button">Log in</div></div>
      <footer><input placeholder="Search"></footer></div>`);
    const { group } = groupFor($('input[type="password"]'), env());
    expect(group.fields.map((f) => f.kind)).toEqual(["username", "password"]);
  });

  it("finds fields inserted after load (nothing is cached)", () => {
    page(`<div id="root"></div>`);
    const root = document.getElementById("root") as HTMLElement;
    const form = document.createElement("form");
    const u = document.createElement("input");
    u.type = "email";
    const p = document.createElement("input");
    p.type = "password";
    form.append(u, p);
    root.append(form);
    expect(groupFor(p, env()).group.fields.map((f) => f.kind)).toEqual(["username", "password"]);
  });

  it("handles fields inside open shadow roots", () => {
    page(`<login-box></login-box>`);
    const host = document.querySelector("login-box") as HTMLElement;
    const shadow = host.attachShadow({ mode: "open" });
    shadow.innerHTML = `<form><input name="u" type="email"><input name="p" type="password"></form>`;
    const pw = shadow.querySelector('[name="p"]') as HTMLInputElement;
    expect(groupFor(pw, env()).group.fields.map((f) => f.kind)).toEqual(["username", "password"]);
  });

  it("attack 7: thousands of inputs cost a bounded amount of work", () => {
    const many = Array.from({ length: 5000 }, (_, i) => `<input name="f${i}">`).join("");
    page(`<form id="big">${many}<input name="user" type="email"><input name="pw" type="password"></form>`);
    const t0 = performance.now();
    const { group } = groupFor($('[name="pw"]'), env());
    const elapsed = performance.now() - t0;
    expect(group.fields.length).toBeLessThanOrEqual(MAX_GROUP_INPUTS);
    expect(elapsed).toBeLessThan(500);
    // A huge wrapper form is not treated as the login form.
    expect(group.root).not.toBe(document.getElementById("big"));
  });
});

describe("filling", () => {
  it("fills username and password with framework-visible events", () => {
    page(`<form><input name="user" type="email"><input name="pw" type="password"></form>`);
    const events: string[] = [];
    for (const n of ["user", "pw"]) {
      const el = $(`[name="${n}"]`);
      el.addEventListener("input", () => events.push(`input:${n}`));
      el.addEventListener("change", () => events.push(`change:${n}`));
    }
    const { group } = groupFor($('[name="pw"]'), env());
    expect(fillLogin(group, { username: "octo", password: "pw-1" }, env())).toBe(2);
    expect($('[name="user"]').value).toBe("octo");
    expect($('[name="pw"]').value).toBe("pw-1");
    expect(events).toEqual(["input:user", "change:user", "input:pw", "change:pw"]);
    // Secrets never appear in attributes.
    expect(document.body.innerHTML).not.toContain("pw-1");
  });

  it("works with React's value tracker (instance-level value property)", () => {
    page(`<form><input name="pw" type="password"></form>`);
    const el = $('[name="pw"]');
    let tracked = "";
    // React installs an own `value` property; the native setter bypasses it,
    // so React sees the change when the input event fires.
    Object.defineProperty(el, "value", {
      configurable: true,
      get: () => tracked,
      set: (v: string) => {
        tracked = v;
      },
    });
    let seen = "";
    el.addEventListener("input", () => {
      seen = (Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.get?.call(el) as string) ?? "";
    });
    const { group } = groupFor(el, env());
    fillLogin(group, { username: null, password: "s3cret" }, env());
    expect(seen).toBe("s3cret");
    expect(tracked).toBe("");
  });

  it("never fills hidden, disabled or readonly fields", () => {
    page(`<form><input name="u" type="email" disabled><input name="p" type="password" readonly></form>`);
    const hiddenPw = $('[name="p"]');
    expect(setValue(hiddenPw, "x", env())).toBe(false);
    expect(setValue($('[name="u"]'), "x", env())).toBe(false);
    page(`<form><input name="p" type="password" style="display:none"></form>`);
    expect(setValue($('[name="p"]'), "x", env())).toBe(false);
    expect($('[name="p"]').value).toBe("");
  });

  it("fills a generated password into new + confirmation only", () => {
    page(`<form><h1>Change password</h1><input name="a" type="password"><input name="b" type="password"><input name="c" type="password"></form>`);
    const { group } = groupFor($('[name="b"]'), env());
    expect(fillNewPassword(group, "Gen-1!", env())).toBe(2);
    expect([$('[name="a"]').value, $('[name="b"]').value, $('[name="c"]').value]).toEqual(["", "Gen-1!", "Gen-1!"]);
  });

  it("fills OTP codes, including split boxes", () => {
    page(`<form><input name="otp" autocomplete="one-time-code"></form>`);
    expect(fillOtp(groupFor($('[name="otp"]'), env()).group, "123456", env())).toBe(1);
    expect($('[name="otp"]').value).toBe("123456");
    page(`<div>${[1, 2, 3, 4, 5, 6].map((i) => `<input name="d${i}" maxlength="1">`).join("")}</div>`);
    expect(fillOtp(groupFor($('[name="d1"]'), env()).group, "987654", env())).toBe(6);
    expect(Array.from(document.querySelectorAll("input"), (i) => i.value).join("")).toBe("987654");
  });

  it("finds the login form on the page for a popup fill", () => {
    page(`<form role="search"><input name="q" type="search"></form>
      <form><input name="user" type="text" autocomplete="username"><input name="pw" type="password"></form>`);
    const group = findLoginGroup(document, env());
    expect(group && fieldsOf(group, "password").map((e) => e.name)).toEqual(["pw"]);
    page(`<form><input name="code" autocomplete="one-time-code"></form>`);
    expect(findLoginGroup(document, env())).toBeNull();
    expect(findOtpGroup(document, env())).not.toBeNull();
  });
});

describe("submissions", () => {
  const group = (sel: string, path = "/") => classifyGroup($(sel).form as HTMLFormElement, env(path));

  it("reads a typed login", () => {
    page(`<form><input name="u" type="email" value=" me@x.com "><input name="p" type="password" value="typed"></form>`);
    markUserEdit($('[name="p"]'));
    expect(readSubmission(group('[name="p"]'))).toEqual({ username: "me@x.com", password: "typed" });
  });

  it("ignores passwords a page script planted or swapped after the user typed", () => {
    page(`<form><input name="u" type="email" value="me"><input name="p" type="password" value="guess"></form>`);
    expect(readSubmission(group('[name="p"]'))).toBeNull();
    const p = $('[name="p"]');
    p.value = "typed";
    markUserEdit(p);
    p.value = "swapped-by-page";
    expect(readSubmission(group('[name="p"]'))).toBeNull();
  });

  it("skips a password we filled and the user did not change", () => {
    page(`<form><input name="u" type="email"><input name="p" type="password"></form>`);
    const g = group('[name="p"]');
    fillLogin(g, { username: "me", password: "from-vault" }, env());
    expect(readSubmission(g)).toBeNull();
    markUserEdit($('[name="p"]'));
    expect(readSubmission(g)).toEqual({ username: "me", password: "from-vault" });
  });

  it("offers generated passwords", () => {
    page(`<form><h1>Sign up</h1><input name="u" type="email" value="me"><input name="p" type="password"><input name="c" type="password"></form>`);
    const g = group('[name="p"]', "/signup");
    fillNewPassword(g, "Gen-2!", env());
    expect(readSubmission(g)).toEqual({ username: "me", password: "Gen-2!" });
  });

  it("drops a new password whose confirmation does not match", () => {
    page(`<form><h1>Sign up</h1><input name="u" type="email" value="me"><input name="p" type="password" value="a"><input name="c" type="password" value="b"></form>`);
    markUserEdit($('[name="p"]'));
    expect(readSubmission(group('[name="p"]', "/signup"))).toBeNull();
  });

  it("username-only steps and empty forms", () => {
    page(`<form><h1>Sign in</h1><input name="u" type="email" autocomplete="username" value="me"></form>`);
    expect(readSubmission(group('[name="u"]'))).toEqual({ username: "me", password: null });
    page(`<form><input name="u" type="email" value="me"><input name="p" type="password"></form>`);
    expect(readSubmission(group('[name="p"]'))).toBeNull();
  });

  it("ignores absurdly long values", () => {
    page(`<form><input name="u" type="email"><input name="p" type="password"></form>`);
    $('[name="p"]').value = "x".repeat(5000);
    markUserEdit($('[name="p"]'));
    expect(readSubmission(group('[name="p"]'))).toBeNull();
  });
});

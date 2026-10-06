// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Env } from "./group";
import { anotherAccountButton, chooserRow, emailIn, findProviderButtons, isConsentScreen, providerButton, MAX_SSO_CANDIDATES } from "./sso";

const env: Env = { isVisible: (el) => !el.hidden && el.style.display !== "none", path: "/login" };
const page = (html: string) => {
  document.body.innerHTML = html; // test fixture only; hygiene.test.ts scans src excluding tests
  return document.body;
};

describe("provider buttons", () => {
  it("recognises common English and Portuguese labels", () => {
    const root = page(`
      <button>Continue with Google</button>
      <a href="https://github.com/login/oauth/authorize?client_id=x">Sign in with GitHub</a>
      <div role="button" aria-label="Entrar com a Microsoft"></div>
      <button><img alt="Apple"> Continuar com Apple</button>`);
    expect([...findProviderButtons(root, env).keys()].sort()).toEqual(["apple", "github", "google", "microsoft"]);
  });
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
  it("takes X from the joiner in any one label source", () => {
    expect(findProviderButtons(page(`<button><img alt="X logo"> Continue with X</button>`), env).has("x")).toBe(true);
    expect(findProviderButtons(page(`<button title="Sign in with X"><img alt="X logo"></button>`), env).has("x")).toBe(true);
    expect(findProviderButtons(page(`<input type="email"><button><img alt="Help with x"> X</button>`), env).has("x")).toBe(false);
  });
  it("ignores footer and social profile links", () => {
    for (const html of [
      `<input type="password"><a href="https://www.linkedin.com/company/acme" aria-label="LinkedIn"></a>`,
      `<input type="password"><a href="https://facebook.com/acme">Facebook</a>`,
      `<input type="password"><a href="https://x.com/acme">Twitter</a>`,
      `<input type="password"><a href="https://github.com/acme">GitHub</a>`,
    ]) expect(findProviderButtons(page(html), env).size).toBe(0);
  });
  it("keeps same-origin and OAuth links", () => {
    expect(findProviderButtons(page(`<input type="password"><a href="/auth/facebook" aria-label="Facebook"></a>`), env).has("facebook")).toBe(true);
    expect(findProviderButtons(page(`<input type="password"><a href="https://www.facebook.com/v19.0/dialog/oauth?client_id=1">Facebook</a>`), env).has("facebook")).toBe(true);
    expect(findProviderButtons(page(`<input type="password"><a href="https://github.com/acme">Sign in with GitHub</a>`), env).has("github")).toBe(true);
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
  it("accepts a bare name only with context", () => {
    expect(findProviderButtons(page(`<button>Google</button>`), env).size).toBe(0);
    expect(findProviderButtons(page(`<button>Google</button><button>GitHub</button>`), env).size).toBe(2);
    expect(findProviderButtons(page(`<input type="email"><button>Google</button>`), env).size).toBe(1);
  });
  it("ignores look-alikes, hidden and disabled buttons", () => {
    const root = page(`
      <a href="https://drive.google.com">Open in Google Drive</a>
      <a href="https://github.com/x/y">Star on GitHub</a>
      <button hidden>Sign in with Google</button>
      <button disabled>Sign in with Apple</button>
      <p>Sign in with Google to continue reading this very long paragraph that is not a button</p>`);
    expect(findProviderButtons(root, env).size).toBe(0);
  });
  it("picks the button to press", () => {
    const root = page(`<button>Continue with Google</button><button>Continue with GitHub</button>`);
    expect(providerButton(root, "github", env)?.textContent).toBe("Continue with GitHub");
    expect(providerButton(root, "apple", env)).toBeNull();
  });
  it("scan_is_bounded", () => {
    const filler = Array.from({ length: MAX_SSO_CANDIDATES + 50 }, (_, i) => `<a href="/p${i}">Page ${i}</a>`).join("");
    const root = page(`${filler}<button>Continue with Google</button>`);
    expect(findProviderButtons(root, env).size).toBe(0);
  });
  it("requires a clear winner over that provider's own runner-up before pressing", () => {
    const tie = page(`<button>Continue with Google</button><button>Continue with Google</button>`);
    expect(providerButton(tie, "google", env)).toBeNull();
    const clear = page(`<input type="email"><button>Continue with Google</button><button>Google</button>`);
    expect(providerButton(clear, "google", env)?.textContent).toBe("Continue with Google");
  });
  it("joins sibling elements' text with a space, so a split word does not fuse", () => {
    // "Continuar com" + "o Google" naively concatenates to "Continuar como Google"
    // ("continue how Google"), which no longer reads as "with the Google".
    const root = page(`<button><span>Continuar com</span><span>o Google</span></button>`);
    expect(findProviderButtons(root, env).get("google")).toBeTruthy();
  });
});

describe("account chooser", () => {
  it("finds the email in a row", () => {
    expect(emailIn(page(`<li><div>Me</div><div>Me@Gmail.com</div></li>`).firstElementChild as Element)).toBe("me@gmail.com");
    expect(emailIn(page(`<li>Use another account</li>`).firstElementChild as Element)).toBeNull();
  });
  it("chooser_row_requires_a_unique_clickable_match", () => {
    const one = page(`<ul><li><div role="link" data-identifier="me@gmail.com">Me <div>me@gmail.com</div></div></li>
      <li><div role="link">You <div>you@gmail.com</div></div></li></ul>`);
    expect(chooserRow(one, "ME@gmail.com", env)?.getAttribute("data-identifier")).toBe("me@gmail.com");
    const two = page(`<div role="link">me@gmail.com</div><div role="link">Signed in as me@gmail.com</div>`);
    expect(chooserRow(two, "me@gmail.com", env)).toBeNull();
    const none = page(`<p>me@gmail.com</p>`);
    expect(chooserRow(none, "me@gmail.com", env)).toBeNull();
  });
  it("sees consent screens", () => {
    expect(isConsentScreen(page(`<p>Typeform wants to access your account</p><button>Allow</button><button>Cancel</button>`), env)).toBe(true);
    expect(isConsentScreen(page(`<button>Continuar</button>`), env)).toBe(true);
    expect(isConsentScreen(page(`<div role="link">me@gmail.com</div><div role="link">Use another account</div>`), env)).toBe(false);
  });
  it("never presses a 'Continue as <account>' tile, however long the account", () => {
    const root = page(`<button>Continue as a.long.name@gmail.com</button>`);
    expect(isConsentScreen(root, env)).toBe(true);
    expect(chooserRow(root, "a.long.name@gmail.com", env)).toBeNull();
  });
  it("does not mistake a name that starts with a consent word for consent", () => {
    const root = page(`<li><div role="link" data-identifier="grant@x.io">Grant Smith <div>grant@x.io</div></div></li>`);
    expect(isConsentScreen(root, env)).toBe(false);
    expect(chooserRow(root, "grant@x.io", env)).not.toBeNull();
  });
  it("never presses a long 'Allow access …' or 'Grant access …' consent button", () => {
    const allow = page(`<button>Allow access for a.long.name@gmail.com</button>`);
    expect(isConsentScreen(allow, env)).toBe(true);
    expect(chooserRow(allow, "a.long.name@gmail.com", env)).toBeNull();

    const grant = page(`<button>Grant access to Some Long Application Name Inc</button>`);
    expect(isConsentScreen(grant, env)).toBe(true);
  });
});

describe("use another account", () => {
  it("finds it in English and Portuguese", () => {
    for (const text of ["Use another account", "Usar outra conta", "Sign in with a different account", "Usar uma conta diferente", "Use a different account", "Entrar com outra conta"]) {
      const root = page(`<ul><li><div role="link">Me <div>me@gmail.com</div></div></li><li><div role="link">${text}</div></li></ul>`);
      expect(anotherAccountButton(root, env)?.textContent).toBe(text);
    }
  });
  it("no_another_account_on_consent", () => {
    const root = page(`<div role="link">Use another account</div><button>Allow</button><button>Cancel</button>`);
    expect(anotherAccountButton(root, env)).toBeNull();
  });
  it("ignores hidden or duplicated controls", () => {
    expect(anotherAccountButton(page(`<div role="link" hidden>Use another account</div>`), env)).toBeNull();
    expect(anotherAccountButton(page(`<div role="link">Use another account</div><a href="#">Use another account</a>`), env)).toBeNull();
  });
});

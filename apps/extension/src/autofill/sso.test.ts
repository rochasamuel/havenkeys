// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Env } from "./group";
import { chooserRow, emailIn, findProviderButtons, isConsentScreen, providerButton, MAX_SSO_CANDIDATES } from "./sso";

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
});

// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Env } from "./group";
import { findSubmitButton, hasChallenge, pressWhenReady } from "./submit";

function visible(el: HTMLElement): boolean {
  for (let n: HTMLElement | null = el; n; n = n.parentElement) {
    if (n.hidden) return false;
    const s = getComputedStyle(n);
    if (s.display === "none" || s.visibility === "hidden") return false;
  }
  return true;
}
const env: Env = { isVisible: visible, path: "/" };
const $ = <T extends Element = HTMLInputElement>(sel: string) => document.querySelector(sel) as unknown as T;
const noSleep = async () => undefined;

beforeEach(() => (document.body.innerHTML = ""));

describe("findSubmitButton", () => {
  it("prefers the form's sign-in button over social and recovery buttons", () => {
    document.body.innerHTML = `<form><input name="u"><input name="p" type="password">
      <button type="button">Sign in with Google</button><a role="button">Forgot password?</a>
      <button type="submit" id="go">Sign in</button></form>`;
    expect(findSubmitButton($("form"), $("[name=p]"), "password", env)?.id).toBe("go");
  });

  it("picks Continue over Create account on a username step", () => {
    document.body.innerHTML = `<div id="root"><input name="email" type="email">
      <button id="next">Continue</button><button>Create account</button></div>`;
    expect(findSubmitButton($("#root"), $("[name=email]"), "username", env)?.id).toBe("next");
  });

  it("finds a form-less button just outside the field's container", () => {
    document.body.innerHTML = `<div><div id="root"><input name="email" type="email"></div>
      <div><button id="next">Next</button></div></div>`;
    expect(findSubmitButton($("#root"), $("[name=email]"), "username", env)?.id).toBe("next");
  });

  it("refuses when two candidates are equally good", () => {
    document.body.innerHTML = `<div id="root"><input name="email" type="email">
      <button>Next</button><button>Continue</button></div>`;
    expect(findSubmitButton($("#root"), $("[name=email]"), "username", env)).toBeNull();
  });

  it("ignores hidden buttons and returns null with nothing that qualifies", () => {
    document.body.innerHTML = `<div id="root"><input name="p" type="password">
      <button hidden>Sign in</button><button>Show</button></div>`;
    expect(findSubmitButton($("#root"), $("[name=p]"), "password", env)).toBeNull();
  });

  it("climbs past a scope where nothing reaches the score threshold", () => {
    document.body.innerHTML = `<div><div id="root"><input name="p" type="password"><button>Show</button></div>
      <div><button id="go">Sign in</button></div></div>`;
    expect(findSubmitButton($("#root"), $("[name=p]"), "password", env)?.id).toBe("go");
  });

  it("scores an input[type=submit] by its value", () => {
    document.body.innerHTML = `<form><input name="otp"><input type="submit" id="v" value="Verify"></form>`;
    expect(findSubmitButton($("form"), $("[name=otp]"), "otp", env)?.id).toBe("v");
  });
});

describe("hasChallenge", () => {
  it("sees a visible reCAPTCHA checkbox, hCaptcha and Turnstile", () => {
    for (const html of [
      `<iframe src="https://www.google.com/recaptcha/api2/anchor?k=x&size=normal"></iframe>`,
      `<iframe src="https://newassets.hcaptcha.com/captcha/v1/x"></iframe>`,
      `<iframe src="https://challenges.cloudflare.com/cdn-cgi/challenge-platform/x"></iframe>`,
      `<div class="cf-turnstile"></div>`,
    ]) {
      document.body.innerHTML = html;
      expect(hasChallenge(document, env), html).toBe(true);
    }
  });

  it("ignores invisible reCAPTCHA badges, hidden widgets and look-alike hosts", () => {
    for (const html of [
      `<iframe src="https://www.google.com/recaptcha/api2/anchor?k=x&size=invisible"></iframe>`,
      `<div class="g-recaptcha" data-size="invisible"></div>`,
      `<iframe hidden src="https://www.google.com/recaptcha/api2/anchor?k=x"></iframe>`,
      `<iframe src="https://hcaptcha.com.evil.example/x"></iframe>`,
      `<iframe src="https://www.google.com/maps"></iframe>`,
      `<button class="g-recaptcha" data-sitekey="x" data-callback="cb" data-action="submit">Sign in</button>`,
    ]) {
      document.body.innerHTML = html;
      expect(hasChallenge(document, env), html).toBe(false);
    }
  });
});

describe("pressWhenReady", () => {
  it("submits the form through requestSubmit with the button as submitter", async () => {
    document.body.innerHTML = `<form><input name="p" type="password"><button id="go">Sign in</button></form>`;
    let submitter: unknown = null;
    $("form").addEventListener("submit", (e) => {
      e.preventDefault();
      submitter = (e as SubmitEvent).submitter;
    });
    const out = await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep });
    expect(out).toBe("pressed");
    expect(submitter).toBe($("#go"));
  });

  it("clicks a form-less button", async () => {
    document.body.innerHTML = `<div><input name="p" type="password"><div role="button" id="go">Log in</div></div>`;
    const click = vi.fn();
    $("#go").addEventListener("click", click);
    await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep });
    expect(click).toHaveBeenCalledOnce();
  });

  it("waits for a disabled button to enable, and gives up after a second", async () => {
    document.body.innerHTML = `<div><input name="p" type="password"><button id="go" disabled>Sign in</button></div>`;
    const go = $<HTMLButtonElement>("#go");
    let slept = 0;
    const sleep = async (ms: number) => {
      slept += ms;
      if (slept === 300) go.disabled = false;
    };
    expect(await pressWhenReady({ button: go, field: $("[name=p]"), step: "password", env, sleep })).toBe("pressed");

    go.disabled = true;
    const never = async () => undefined;
    expect(await pressWhenReady({ button: go, field: $("[name=p]"), step: "password", env, sleep: never })).toBe("gave_up");
  });

  it("treats aria-disabled like disabled", async () => {
    document.body.innerHTML = `<div><input name="p" type="password"><button id="go" aria-disabled="true">Sign in</button></div>`;
    expect(await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep })).toBe("gave_up");
  });

  it("does not press when the site submitted the code by itself", async () => {
    document.body.innerHTML = `<form><input name="otp"><button id="v">Verify</button></form>`;
    const click = vi.fn();
    $("#v").addEventListener("click", click);
    const sleep = async () => $("[name=otp]").remove();
    expect(await pressWhenReady({ button: $("#v"), field: $("[name=otp]"), step: "otp", env, sleep })).toBe("site_submitted");
    expect(click).not.toHaveBeenCalled();
  });

  it("does not press while a challenge is showing", async () => {
    document.body.innerHTML = `<form><input name="p" type="password"><div class="h-captcha"></div><button id="go">Sign in</button></form>`;
    expect(await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep })).toBe("gave_up");
  });

  it("presses a sign-in button that is also Google's invisible reCAPTCHA binding", async () => {
    document.body.innerHTML = `<form><input name="p" type="password">
      <button class="g-recaptcha" data-sitekey="x" data-callback="cb" data-action="submit" id="go">Sign in</button></form>`;
    $("form").addEventListener("submit", (e) => e.preventDefault());
    expect(await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep })).toBe("pressed");
  });
});

// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { groupRoot, type Env } from "./group";
import { BUTTON_WAIT_MS, findSubmitButton, hasChallenge, pressWhenReady, waitForSubmitButton } from "./submit";

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
const deep = (inner: string, n: number) => "<div>".repeat(n) + inner + "</div>".repeat(n);

beforeEach(() => (document.body.innerHTML = ""));

describe("findSubmitButton", () => {
  it("gov.br: Continuar on the CPF step, Entrar (not Cancelar / Esqueci minha senha) on the password step", () => {
    document.body.innerHTML = `<form><input id="accountId" name="accountId" autocomplete="new-password" type="tel" inputmode="numeric" placeholder="Digite seu CPF">
      <div class="button-panel"><button id="enter-account-id" type="submit" name="operation" value="enter-account-id">Continuar</button></div></form>`;
    expect(findSubmitButton($("form"), $("#accountId"), "username", env)?.id).toBe("enter-account-id");

    document.body.innerHTML = `<form><h3>Digite sua senha</h3>
      <div class="password-eye"><input name="password" id="password" type="password" placeholder="Digite sua senha atual" autocomplete="new-password">
      <span tabindex="2" class="fa fa-fw fa-eye toggle-password"></span></div>
      <div class="actions"><div class="button-panel">
        <button type="button" value="cancel">Cancelar</button>
        <button id="submit-button" type="submit" name="operation" value="enter-password" aria-label="Botão Entrar. Aperte a tecla enter para entrar.">Entrar</button>
      </div><button id="password-recovery" type="button" name="operation" value="call-account-recovery">Esqueci minha senha</button></div></form>`;
    expect(findSubmitButton($("form"), $("#password"), "password", env)?.id).toBe("submit-button");
  });

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

  it("Google (Firefox): finds Avançar far from a form-less field, not Esqueceu o e-mail? / Criar conta", () => {
    document.body.innerHTML = `<main>${deep(`<section><div id="root">${deep('<input id="identifierId" type="text" autocomplete="username">', 4)}</div></section>`, 5)}
      ${deep('<button type="button">Esqueceu o e-mail?</button>', 3)}
      ${deep('<div id="identifierNext"><button id="next" type="button"><span>Avançar</span></button></div>', 3)}
      ${deep('<button type="button">Criar conta</button>', 3)}</main>`;
    expect(findSubmitButton($("#root"), $("#identifierId"), "username", env)?.id).toBe("next");
  });

  it("Google (Firefox): Avançar on the password step too, not Esqueceu a senha?", () => {
    document.body.innerHTML = `<main>${deep(`<section><div id="root">${deep('<input id="pw" type="password" name="Passwd" autocomplete="current-password">', 4)}
      <input type="checkbox" id="show"><label for="show">Mostrar senha</label></div></section>`, 5)}
      ${deep('<div id="passwordNext"><button id="next" type="button"><span>Avançar</span></button></div>', 3)}
      ${deep('<button type="button">Esqueceu a senha?</button>', 3)}</main>`;
    expect(findSubmitButton($("#root"), $("#pw"), "password", env)?.id).toBe("next");
  });

  // The structure of Google's OAuth password page (Firefox), values left out.
  const googlePassword = (captcha: string) => `<main>
    <div><h1 id="headingText"><span>Hi</span></h1><div data-email="me@example.com">me@example.com</div></div>
    <div><div><div><div><span>
      <section><div>To continue, first verify it's you</div></section>
      <section aria-hidden="true" style="display:none">Too many failed attempts</section>
      <section><header aria-hidden="true"></header><div><div>
        <input type="email" name="identifier" style="display:none" tabindex="-1" aria-hidden="true" id="hiddenEmail">
        <div class="sWKwnd">${deep('<div id="password">' + deep('<input id="pw" type="password" name="Passwd" autocomplete="current-password webauthn">', 3) + "</div>", 4)}
          <div id="c7"></div>
          <div>${deep('<input type="checkbox" aria-labelledby="sel"><div id="sel">Show password</div>', 4)}</div>
        </div>
        <input type="hidden" name="TrustDevice">
        <div ${captcha}><img id="captchaimg"><div id="playCaptchaButton"><button type="button" aria-label="Listen and type the numbers you hear"></button></div>
          <div>${deep('<input type="text" name="ca" id="ca" autocomplete="off">', 4)}</div></div>
      </div></div></section>
      <input type="hidden" id="identifierId">
    </span></div><div><div></div></div></div></div></div>
    <div><div><div><div id="passwordNext"><div><button id="next" type="button"><span>Next</span></button></div></div>
      <div><div><button type="button"><span>Try another way</span></button></div></div></div></div>
      <div aria-hidden="true"><div><button aria-label="Scroll down" type="button"></button></div></div></div></main>`;

  it("Google OAuth password page: presses Next from the field's own group", () => {
    document.body.innerHTML = googlePassword('style="display:none"');
    const pw = $("#pw");
    expect(findSubmitButton(groupRoot(pw), pw, "password", env)?.id).toBe("next");
  });

  it("Google OAuth password page with its image CAPTCHA shown: no press", () => {
    document.body.innerHTML = googlePassword("");
    const pw = $("#pw");
    expect(findSubmitButton(groupRoot(pw), pw, "password", env)).toBeNull();
  });

  it("does not climb far into a scope holding another field (another form's button)", () => {
    document.body.innerHTML = `<main>${deep(`<div id="root"><input id="u" type="text" autocomplete="username"></div>`, 6)}
      ${deep('<input type="email" name="newsletter"><button id="sub">Continuar</button>', 2)}</main>`;
    expect(findSubmitButton($("#root"), $("#u"), "username", env)).toBeNull();
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

describe("waitForSubmitButton", () => {

  it("Google: finds Avançar once the outgoing email view's own Avançar is gone", async () => {
    document.body.innerHTML = `<main>
      <div id="old">${deep('<div id="identifierNext"><button id="old-next" type="button">Avançar</button></div>', 3)}</div>
      ${deep(`<div id="root">${deep('<input id="pw" type="password" name="Passwd" autocomplete="current-password">', 4)}</div>`, 5)}
      ${deep('<div id="passwordNext"><button id="next" type="button">Avançar</button></div>', 3)}</main>`;
    // Both views on screen: a tie, no button.
    expect(findSubmitButton($("#root"), $("#pw"), "password", env)).toBeNull();
    let polls = 0;
    const sleep = async () => {
      if (++polls === 2) $("#old").remove();
    };
    const b = await waitForSubmitButton({ root: $("#root"), field: $("#pw"), step: "password", env: () => env, sleep });
    expect(b?.id).toBe("next");
  });

  it("finds a button rendered after the field", async () => {
    document.body.innerHTML = `<main>${deep(`<div id="root">${deep('<input id="pw" type="password">', 4)}</div>`, 5)}<div id="footer"></div></main>`;
    const sleep = async () => {
      $("#footer").innerHTML = '<button id="next" type="button">Avançar</button>';
    };
    const b = await waitForSubmitButton({ root: $("#root"), field: $("#pw"), step: "password", env: () => env, sleep });
    expect(b?.id).toBe("next");
  });

  it("gives up after BUTTON_WAIT_MS", async () => {
    document.body.innerHTML = `<div id="root"><input id="pw" type="password"></div>`;
    let slept = 0;
    const sleep = async (ms: number) => void (slept += ms);
    expect(await waitForSubmitButton({ root: $("#root"), field: $("#pw"), step: "password", env: () => env, sleep })).toBeNull();
    expect(slept).toBe(BUTTON_WAIT_MS);
  });

  it("stops when cancelled or when the field leaves the page", async () => {
    document.body.innerHTML = `<div id="root"><input id="pw" type="password"></div><div id="footer"></div>`;
    let cancel = false;
    const sleep = async () => {
      cancel = true;
      $("#footer").innerHTML = '<button type="button">Sign in</button>';
    };
    expect(await waitForSubmitButton({ root: $("#root"), field: $("#pw"), step: "password", env: () => env, sleep, cancelled: () => cancel })).toBeNull();

    document.body.innerHTML = `<div id="root"><input id="pw" type="password"></div><div id="footer"></div>`;
    const field = $("#pw");
    const gone = async () => {
      field.remove();
      $("#footer").innerHTML = '<button type="button">Sign in</button>';
    };
    expect(await waitForSubmitButton({ root: $("#root"), field, step: "password", env: () => env, sleep: gone })).toBeNull();
  });
});

describe("pressWhenReady", () => {
  it("submits the form with the button as submitter", async () => {
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

  // gov.br starts its (invisible hCaptcha) submit from the button's click
  // listener and cancels the native submit; requestSubmit alone skips it.
  it("runs the button's own click handler for a form's submit button", async () => {
    document.body.innerHTML = `<form><input name="u" type="tel"><button id="go" type="submit">Continuar</button></form>`;
    const click = vi.fn((e: Event) => e.preventDefault());
    const submit = vi.fn((e: Event) => e.preventDefault());
    $("#go").addEventListener("click", click);
    $("form").addEventListener("submit", submit);
    await pressWhenReady({ button: $("#go"), field: $("[name=u]"), step: "username", env, sleep: noSleep });
    expect(click).toHaveBeenCalledOnce();
    expect(submit).not.toHaveBeenCalled();
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
    const click = vi.fn();
    go.addEventListener("click", click);
    const never = async () => undefined;
    expect(await pressWhenReady({ button: go, field: $("[name=p]"), step: "password", env, sleep: never })).toBe("gave_up");
    expect(click).not.toHaveBeenCalled();
  });

  it("gives up without pressing once cancelled during the disabled-button wait", async () => {
    document.body.innerHTML = `<div><input name="p" type="password"><button id="go" disabled>Sign in</button></div>`;
    const go = $<HTMLButtonElement>("#go");
    const click = vi.fn();
    go.addEventListener("click", click);
    let cancelled = false;
    const sleep = async () => {
      // The run ends, and only then (as on a real page) the button enables:
      // without the cancelled check this would proceed to press it.
      cancelled = true;
      go.disabled = false;
    };
    const out = await pressWhenReady({
      button: go,
      field: $("[name=p]"),
      step: "password",
      env,
      sleep,
      cancelled: () => cancelled,
    });
    expect(out).toBe("gave_up");
    expect(click).not.toHaveBeenCalled();
  });

  it("treats aria-disabled like disabled", async () => {
    document.body.innerHTML = `<div><input name="p" type="password"><button id="go" aria-disabled="true">Sign in</button></div>`;
    const click = vi.fn();
    $("#go").addEventListener("click", click);
    expect(await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep })).toBe("gave_up");
    expect(click).not.toHaveBeenCalled();
  });

  it("treats a button inside a disabled fieldset as disabled", async () => {
    document.body.innerHTML = `<form><fieldset disabled><input name="p" type="password"><button id="go">Sign in</button></fieldset></form>`;
    const submitted = vi.fn((e: Event) => e.preventDefault());
    const click = vi.fn();
    $("form").addEventListener("submit", submitted);
    $("#go").addEventListener("click", click);
    expect(await pressWhenReady({ button: $("#go"), field: $("[name=p]"), step: "password", env, sleep: noSleep })).toBe("gave_up");
    expect(submitted).not.toHaveBeenCalled();
    expect(click).not.toHaveBeenCalled();
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

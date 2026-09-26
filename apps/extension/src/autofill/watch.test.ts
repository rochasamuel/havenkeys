// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setValue } from "./fill";
import type { Env } from "./group";
import { CHECK_DEBOUNCE_MS, detectStep, watchNext, WATCH_TIMEOUT_MS, type WatchResult } from "./watch";

function visible(el: HTMLElement): boolean {
  for (let n: HTMLElement | null = el; n; n = n.parentElement) {
    if (n.hidden) return false;
    const s = getComputedStyle(n);
    if (s.display === "none" || s.visibility === "hidden") return false;
  }
  return true;
}
const env: Env = { isVisible: visible, path: "/" };
const $ = (sel: string) => document.querySelector(sel) as HTMLInputElement;

beforeEach(() => {
  vi.useFakeTimers();
  document.body.innerHTML = "";
});
afterEach(() => vi.useRealTimers());

function watch(want: "password" | "otp", filled: HTMLInputElement | null = null) {
  const results: WatchResult[] = [];
  let checks = 0;
  const cancel = watchNext({ want, filled, env: () => (checks++, env), onResult: (r) => results.push(r) });
  return { results, cancel, checks: () => checks };
}

describe("detectStep", () => {
  it("finds a password page and an OTP page", () => {
    document.body.innerHTML = `<form><input name="p" type="password"><button>Sign in</button></form>`;
    expect(detectStep(document, env, "password", null)?.kind).toBe("password");
    document.body.innerHTML = `<form><input name="otp" autocomplete="one-time-code"></form>`;
    expect(detectStep(document, env, "otp", null)?.kind).toBe("otp");
  });

  it("keeps waiting while the pressed field is still on screen with our value (SPA in flight)", () => {
    document.body.innerHTML = `<form><input name="email" type="email" autocomplete="username"><button>Continue</button></form>`;
    setValue($("[name=email]"), "a@b.c", env);
    expect(detectStep(document, env, "password", $("[name=email]"))).toBeNull();
  });

  it("reports came_back when the password page returns emptied or on a fresh page", () => {
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    const pw = $("[name=p]");
    setValue(pw, "secret", env);
    expect(detectStep(document, env, "otp", pw)).toBeNull();
    pw.value = ""; // the site cleared it after "wrong password"
    expect(detectStep(document, env, "otp", pw)?.kind).toBe("came_back");
    expect(detectStep(document, env, "otp", null)?.kind).toBe("came_back");
  });

  it("reports a visible challenge", () => {
    document.body.innerHTML = `<div class="h-captcha"></div><form><input name="p" type="password"></form>`;
    expect(detectStep(document, env, "password", null)?.kind).toBe("challenge");
  });
});

describe("watchNext", () => {
  it("reports a field present at start once it is stable", async () => {
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    const w = watch("password");
    expect(w.results).toEqual([]);
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    expect(w.results).toEqual([{ found: "password", field: $("[name=p]") }]);
  });

  it("reports a field inserted later and one revealed by a class change", async () => {
    const w = watch("otp");
    await vi.advanceTimersByTimeAsync(1000);
    document.body.innerHTML = `<style>.off{display:none}</style><form class="off"><input name="otp" autocomplete="one-time-code"></form>`;
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS * 3);
    expect(w.results).toEqual([]);
    document.querySelector("form")?.classList.remove("off");
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS * 3);
    expect(w.results).toEqual([{ found: "otp", field: $("[name=otp]") }]);
  });

  it("ignores a field that shows for one check only", async () => {
    const w = watch("password");
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    document.body.innerHTML = "";
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS * 3);
    expect(w.results).toEqual([]);
    w.cancel();
  });

  it("stops on came_back, on a challenge, and after the timeout", async () => {
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    const back = watch("otp", null);
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    expect(back.results).toEqual([{ stop: "came_back" }]);

    document.body.innerHTML = `<div class="cf-turnstile"></div>`;
    const ch = watch("password");
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    expect(ch.results).toEqual([{ stop: "challenge" }]);

    document.body.innerHTML = "";
    const idle = watch("password");
    await vi.advanceTimersByTimeAsync(WATCH_TIMEOUT_MS);
    expect(idle.results).toEqual([{ stop: "timeout" }]);
  });

  it("reports nothing after cancel", async () => {
    const w = watch("password");
    w.cancel();
    document.body.innerHTML = `<form><input name="p" type="password"></form>`;
    await vi.advanceTimersByTimeAsync(WATCH_TIMEOUT_MS);
    expect(w.results).toEqual([]);
  });

  it("stays bounded under a mutation storm (attack 7)", async () => {
    const w = watch("password");
    for (let i = 0; i < 50; i++) {
      const frag = document.createDocumentFragment();
      for (let j = 0; j < 100; j++) frag.appendChild(document.createElement("input"));
      document.body.appendChild(frag);
      await vi.advanceTimersByTimeAsync(10);
    }
    await vi.advanceTimersByTimeAsync(CHECK_DEBOUNCE_MS);
    // 5,000 inputs over ~500 ms: one check per debounce window, not per mutation.
    expect(w.checks()).toBeLessThanOrEqual(1 + Math.ceil(700 / CHECK_DEBOUNCE_MS) + 1);
    w.cancel();
  });
});

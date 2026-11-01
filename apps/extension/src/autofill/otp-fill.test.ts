// @vitest-environment jsdom
//
// Split one-time-code widgets as the popular libraries build them (markup and
// event handling read from their published sources, 2026-10): React and Vue
// apply an event's state change in a later microtask, so a box written
// before the page settles from the last one is lost or lands in the wrong box.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Env } from "./group";
import { fieldsOf, groupFor } from "./group";
import { fillOtp } from "./fill";

const env: Env = { isVisible: () => true, path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const boxes = () => Array.from(document.querySelectorAll<HTMLInputElement>("#otp input"));
const shown = () => boxes().map((b) => b.value).join("");
const fill = (code: string) => fillOtp(groupFor(boxes()[0] as HTMLInputElement, env).group, code, env);
const render = (box: (i: number) => string, n = 6) =>
  page(`<form><p>Enter the code</p><div id="otp">${Array.from({ length: n }, (_, i) => box(i)).join("")}</div><button type="submit">Verify</button></form>`);

/** Jsdom has neither: enough of each for a page's paste handler. */
class FakeDataTransfer {
  private data = new Map<string, string>();
  setData(t: string, v: string) {
    this.data.set(t, v);
  }
  getData(t: string) {
    return this.data.get(t) ?? "";
  }
}
class FakeClipboardEvent extends Event {
  clipboardData: FakeDataTransfer | null;
  constructor(type: string, init: EventInit & { clipboardData?: FakeDataTransfer }) {
    super(type, init);
    this.clipboardData = init.clipboardData ?? null;
  }
}
beforeEach(() => {
  vi.stubGlobal("DataTransfer", FakeDataTransfer);
  vi.stubGlobal("ClipboardEvent", FakeClipboardEvent);
});
afterEach(() => vi.unstubAllGlobals());

/**
 * A controlled widget: the boxes show `state`, and handlers see the state of
 * the last render, which happens in a microtask after a change.
 */
function widget(onInput: (s: { code: string[]; active: number }, i: number, v: string) => void, onPaste?: (s: { code: string[]; active: number }, text: string) => void) {
  const state = { code: Array<string>(boxes().length).fill(""), active: 0 };
  let seen = { code: [...state.code], active: 0 };
  const rerender = () =>
    queueMicrotask(() => {
      seen = { code: [...state.code], active: state.active };
      boxes().forEach((b, i) => (b.value = state.code[i] ?? ""));
      boxes()[state.active]?.focus();
    });
  boxes().forEach((b, i) => {
    b.addEventListener("focus", () => {
      state.active = i;
      queueMicrotask(() => (seen = { ...seen, active: i }));
    });
    b.addEventListener("input", () => {
      Object.assign(state, { code: [...seen.code], active: seen.active });
      onInput(state, i, b.value);
      rerender();
    });
    if (onPaste)
      b.addEventListener("paste", (e) => {
        e.preventDefault();
        Object.assign(state, { code: [...seen.code], active: seen.active });
        onPaste(state, (e as unknown as FakeClipboardEvent).clipboardData?.getData("text/plain") ?? "");
        rerender();
      });
  });
}

describe("split one-time codes", () => {
  it("react-otp-input: the typed character goes to the box the widget saw focused", async () => {
    render((i) => `<input aria-label="Please enter OTP character ${i + 1}" autocomplete="off" inputmode="numeric" type="text" style="width:1em">`);
    widget((s, _i, v) => {
      s.code[s.active] = v.slice(-1);
      s.active = Math.min(s.active + 1, 5);
    });
    expect(fieldsOf(groupFor(boxes()[0] as HTMLInputElement, env).group, "otp")).toHaveLength(6);
    expect(await fill("987654")).toBe(6);
    expect(shown()).toBe("987654");
  });

  it("MUI OtpInput: each box rebuilds the code from the last render", async () => {
    render(() => `<div><input autocomplete="one-time-code" type="text" class="MuiInputBase-input"></div>`);
    widget((s, i, v) => {
      if (i === 0 && v.length > 1) s.code = v.split("").slice(0, 6);
      else s.code[i] = v[0] ?? "";
    });
    expect(await fill("204517")).toBe(6);
    expect(shown()).toBe("204517");
  });

  it("vue3-otp-input: only the last box has a maxlength", async () => {
    render((i) => `<input type="tel" inputmode="numeric" class="otp-input"${i === 5 ? ' maxlength="1"' : ""}>`);
    expect(await fill("135790")).toBe(6);
    expect(shown()).toBe("135790");
  });

  it("a widget that only takes a pasted code gets a paste", async () => {
    render(() => `<input maxlength="1" class="digit">`);
    widget(
      () => {},
      (s, text) => {
        s.code = text.slice(0, 6).split("");
      },
    );
    expect(await fill("112233")).toBe(6);
    expect(shown()).toBe("112233");
  });

  it("a widget that only splits a whole code in its first box gets that", async () => {
    render(() => `<input autocomplete="one-time-code" class="b">`);
    widget((s, i, v) => {
      if (i === 0 && v.length === 6) s.code = v.split("");
    });
    expect(await fill("445566")).toBe(6);
    expect(shown()).toBe("445566");
  });

  it("a widget that takes nothing is left without the whole code in one box", async () => {
    render(() => `<input maxlength="1" class="digit">`);
    widget(() => {});
    expect(await fill("778899")).toBe(0);
    expect(boxes().every((b) => b.value.length <= 1)).toBe(true);
  });

  it("look-alike boxes nothing names as a code are not one", () => {
    render(() => `<input type="text" class="field">`);
    expect(fieldsOf(groupFor(boxes()[0] as HTMLInputElement, env).group, "otp")).toEqual([]);
  });

  it("a single code field still takes the whole code", async () => {
    page(`<form><div id="otp"><input autocomplete="one-time-code" maxlength="6" inputmode="numeric"></div></form>`);
    expect(await fill("123456")).toBe(1);
    expect(shown()).toBe("123456");
  });
});

// @vitest-environment jsdom
//
// Deterministic fuzz of the extension's untrusted-input handling (CLAUDE.md
// §47): message validators, URL stripping, text normalization, and field
// classification over randomly generated hostile DOM.

import { describe, expect, it } from "vitest";
import { classifyGroup, groupFor, MAX_GROUP_INPUTS, type Env } from "./autofill/group";
import { readSubmission } from "./autofill/page";
import { normalize } from "./autofill/text";
import { parseBackgroundMessage, parseContentRequest, parseInlineRequest } from "./messaging/inline";
import { parsePopupRequest } from "./messaging/popup";
import { pageUrlForRequest } from "./shared/url";

function rng(seed: number) {
  let s = seed >>> 0 || 1;
  const next = () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return (s >>> 0) / 0x1_0000_0000;
  };
  return {
    next,
    int: (n: number) => Math.floor(next() * n),
    pick: <T>(xs: readonly T[]): T => xs[Math.floor(next() * xs.length)] as T,
    str(max: number): string {
      const chars = "aZ09 _-./:@?#%\\\"'<>&\u0000‮​çã한😀\n\t";
      return Array.from({ length: Math.floor(next() * max) }, () => chars[Math.floor(next() * chars.length)]).join("");
    },
  };
}

const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const TOKEN = "0123456789abcdef0123456789abcdef";

describe("message validators", () => {
  const seeds: Record<string, unknown>[] = [
    { type: "cs_open_menu", kind: "login" },
    { type: "cs_submit", username: "u", password: "p" },
    { type: "cs_ready" },
    { type: "cs_close_menu", token: TOKEN },
    { type: "menu_pick", token: TOKEN, itemId: ID },
    { type: "save_confirm", token: TOKEN },
    { type: "cs_run_step", kind: "password" },
    { type: "cs_run_step", kind: "otp" },
    { type: "cs_run_stop" },
    { type: "bg_fill", origin: "https://a.com", token: null, fill: { kind: "login", username: "u", password: "p" }, submit: true, totp: true },
    { type: "bg_fill", origin: "https://a.com", token: TOKEN, fill: { kind: "otp", code: "123456" }, submit: false, totp: false },
    { type: "bg_run_end" },
    { type: "popup_fill", itemId: ID },
  ];
  const atoms: unknown[] = [null, 0, -1, "", "x".repeat(5000), ID, TOKEN, [], {}, true, "https://evil.com", "__proto__"];

  const parsers = [parseContentRequest, parseInlineRequest, parseBackgroundMessage, parsePopupRequest];

  it("every seed is a valid message for some parser (the corpus is not stale)", () => {
    for (const seed of seeds) expect(parsers.some((parse) => parse(seed) !== null)).toBe(true);
  });

  it("never throw, and accept only messages with exactly the expected keys", () => {
    const r = rng(0xbeef);
    for (let i = 0; i < 20_000; i++) {
      const msg: Record<string, unknown> = { ...r.pick(seeds) };
      const keys = Object.keys(msg);
      for (let m = 0; m <= r.int(3); m++) {
        const roll = r.next();
        if (roll < 0.3) msg[r.pick(["extra", "url", "origin", "tabId"])] = r.pick(atoms);
        else if (roll < 0.5) delete msg[r.pick(keys)];
        else msg[r.pick(keys)] = roll < 0.8 ? r.pick(atoms) : r.str(40);
      }
      for (const parse of parsers) {
        let out: unknown = null;
        expect(() => (out = parse(msg))).not.toThrow();
        // Anything accepted carries no key the sender invented.
        if (out) expect(Object.keys(out as object).sort()).toEqual(Object.keys(msg).sort());
      }
    }
  });
});

describe("URL stripping", () => {
  it("never throws; accepted URLs are bare http(s) under the size limit", () => {
    const r = rng(0x0421);
    const seeds = ["https://u:p@a.com:8443/x?y#z", "http://[::1]/", "javascript:alert(1)", "https://xn--e1a.com/", "data:,x"];
    for (let i = 0; i < 20_000; i++) {
      const s = r.next() < 0.3 ? r.str(80) : r.pick(seeds) + r.str(20);
      const out = pageUrlForRequest(s);
      if (out === null) continue;
      const u = new URL(out);
      expect(["https:", "http:"]).toContain(u.protocol);
      expect(u.username + u.password + u.search + u.hash).toBe("");
      expect(new TextEncoder().encode(out).length).toBeLessThanOrEqual(4096);
    }
  });

  it("normalize is bounded and total", () => {
    const r = rng(7);
    for (let i = 0; i < 5_000; i++) {
      const out = normalize(r.str(400));
      expect(out.length).toBeLessThanOrEqual(400);
      expect(out).toMatch(/^[a-z0-9 ]*$/);
    }
  });
});

describe("classification over hostile DOM", () => {
  const env: Env = { isVisible: () => true, path: "/" };
  const types = ["text", "email", "password", "tel", "number", "hidden", "search", "checkbox", "", "PASSWORD", "x-weird"];
  const attrs = ["name", "id", "placeholder", "aria-label", "title", "autocomplete", "inputmode", "maxlength", "aria-labelledby"];
  const words = ["username", "password", "email", "otp", "code", "confirm", "new", "current", "search", "one-time-code", "new-password", "current-password", "cc-number", "senha", "-1", "999999"];

  it("never throws, stays bounded, and yields at most one username per group", () => {
    const r = rng(0xd0e);
    for (let round = 0; round < 300; round++) {
      document.body.replaceChildren();
      const root = r.next() < 0.5 ? document.createElement("form") : document.createElement("div");
      document.body.append(root);
      let parent: HTMLElement = root;
      const n = 1 + r.int(80);
      for (let i = 0; i < n; i++) {
        if (r.next() < 0.2) {
          const wrap = document.createElement(r.pick(["div", "label", "span", "fieldset"]));
          wrap.textContent = r.next() < 0.5 ? r.pick(words) : r.str(30);
          parent.append(wrap);
          if (r.next() < 0.5) parent = wrap;
        }
        const input = document.createElement("input");
        input.setAttribute("type", r.pick(types));
        for (let a = 0; a < r.int(4); a++) {
          input.setAttribute(r.pick(attrs), r.next() < 0.6 ? r.pick(words) : r.str(300));
        }
        if (r.next() < 0.1) input.disabled = true;
        parent.append(input);
      }
      const inputs = Array.from(document.querySelectorAll("input"));
      const field = r.pick(inputs);
      let out: ReturnType<typeof groupFor> | undefined;
      expect(() => (out = groupFor(field, env))).not.toThrow();
      const { group } = out as ReturnType<typeof groupFor>;
      expect(group.fields.length).toBeLessThanOrEqual(MAX_GROUP_INPUTS);
      expect(group.fields.filter((f) => f.kind === "username").length).toBeLessThanOrEqual(1);
      for (const f of group.fields) {
        expect(f.confidence).toBeGreaterThanOrEqual(0);
        expect(f.confidence).toBeLessThanOrEqual(1);
        expect(f.el.disabled).toBe(false);
      }
      expect(() => readSubmission(classifyGroup(root, env))).not.toThrow();
    }
  });
});

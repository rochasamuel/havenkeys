// @vitest-environment jsdom
//
// The shared Brazilian field fixture (packages/field-words/fixtures,
// spec 2026-10-19-shared-field-words-design §6): each real field, built on
// its own with its label, must get the role the fixture names from each
// classifier. Android's BrFieldsTest runs the same records.

import { describe, expect, test } from "vitest";
import fixture from "../../../../packages/field-words/fixtures/br-fields.json";
import { cardKindOf } from "./card-kind";
import { classifyPasswords, OTP_THRESHOLD, otpScore, USERNAME_ALONE_THRESHOLD, USERNAME_THRESHOLD, usernameScore, type GroupIntent } from "./classify";
import { features } from "./group";
import { identityRoleOf } from "./identity";

type Classifier = "login" | "identity" | "card";
type Rec = (typeof fixture.records)[number] & {
  context?: { hasPassword: boolean; lastBeforePassword: boolean; loginIntent: boolean };
};

const ATTRS = ["type", "name", "id", "placeholder", "autocomplete", "inputmode", "maxlength"] as const;

function build(r: Rec): HTMLInputElement | HTMLSelectElement {
  document.body.textContent = "";
  const el = document.createElement(r.tag === "select" ? "select" : "input");
  for (const a of ATTRS) if (r[a] && !(a === "type" && r.tag === "select")) el.setAttribute(a, r[a]);
  if (r.aria_label) el.setAttribute("aria-label", r.aria_label);
  if (!el.id) el.id = "field";
  const label = document.createElement("label");
  label.htmlFor = el.id;
  label.textContent = r.label_text;
  document.body.append(label, el);
  return el;
}

const PASSWORD_INTENT: Record<string, GroupIntent> = { signup: "signup", reset: "change" };

function loginRole(r: Rec, el: HTMLInputElement): string {
  if (cardKindOf(el, false)) return "none";
  const f = features(el);
  if (f.type === "password") {
    const kind = classifyPasswords([f], PASSWORD_INTENT[r.page_kind] ?? "login")[0]?.kind ?? "unknown";
    return kind === "current-password" ? "password" : kind === "unknown" ? "none" : kind;
  }
  const ctx = r.context ?? { hasPassword: true, lastBeforePassword: false, loginIntent: false };
  if (usernameScore(f, ctx) >= (ctx.hasPassword ? USERNAME_THRESHOLD : USERNAME_ALONE_THRESHOLD)) return "username";
  if (otpScore(f, false) >= OTP_THRESHOLD) return "otp";
  return "none";
}

function actual(r: Rec, c: Classifier): string {
  const el = build(r);
  if (c === "card") return cardKindOf(el, false)?.kind ?? "none";
  if (c === "identity") return identityRoleOf(el)?.role ?? "none";
  return loginRole(r, el as HTMLInputElement);
}

const gaps = new Set(
  (fixture.known_gaps as Array<{ site: string; id: string; label_text: string; platform: string; classifier: string }>)
    .filter((g) => g.platform === "extension")
    .map((g) => `${g.site}|${g.id}|${g.label_text}|${g.classifier}`),
);

const cases = (fixture.records as Rec[]).flatMap((r) =>
  (["login", "identity", "card"] as const)
    .filter((c) => r[c] !== "skip")
    .map((c) => ({ r, c, name: `${r.site} · ${r.label_text || r.name || r.id} · ${c}` })),
);

describe("Brazilian field fixture", () => {
  test.each(cases)("$name", ({ r, c }) => {
    const got = actual(r, c);
    // A known gap must still fail, so a fix forces removing its entry.
    if (gaps.has(`${r.site}|${r.id}|${r.label_text}|${c}`)) expect(got).not.toBe(r[c]);
    else expect(got).toBe(r[c]);
  });
});

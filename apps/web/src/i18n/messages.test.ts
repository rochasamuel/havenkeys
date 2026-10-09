import { describe, expect, it } from "vitest";
import { en } from "./en";
import { ptBR } from "./pt-BR";

const TECH = /argon2|hkdf|aead|ciphertext|rust core|xchacha|aes-/i;

describe("messages", () => {
  it("has six main features on Home in both languages", () => {
    expect(en.home.cards).toHaveLength(6);
    expect(ptBR.home.cards).toHaveLength(6);
  });
  it("keeps jargon off the everyday pages", () => {
    for (const m of [en, ptBR]) {
      const plain = JSON.stringify([m.home, m.security]);
      expect(plain).not.toMatch(TECH);
    }
  });
  it("sends the terms version that is on the Terms page", async () => {
    const { TERMS_VERSION, TERMS_UPDATED, PRIVACY_UPDATED } = await import("../lib/legal");
    expect(TERMS_VERSION).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(en.terms.updated).toBe(TERMS_UPDATED.en);
    expect(ptBR.terms.updated).toBe(TERMS_UPDATED["pt-BR"]);
    expect(en.privacy.updated).toBe(PRIVACY_UPDATED.en);
    expect(ptBR.privacy.updated).toBe(PRIVACY_UPDATED["pt-BR"]);
    // "October 20, 2026" <-> "2026-10-20": the day and year must appear.
    const [y, , d] = TERMS_VERSION.split("-");
    expect(en.terms.updated).toContain(`${Number(d)}, ${y}`);
  });
  it("has no request-an-invite copy left", () => {
    for (const m of [en, ptBR]) {
      expect(JSON.stringify(m)).not.toMatch(/invite@havenkeys\.net|Request an invite|Pedir um convite/);
    }
  });
});

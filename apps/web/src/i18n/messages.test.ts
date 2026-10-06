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
  it("uses the invite subjects from the spec", () => {
    expect(en.common.inviteSubject).toBe("HavenKeys invite request");
    expect(ptBR.common.inviteSubject).toBe("Pedido de convite HavenKeys");
  });
});

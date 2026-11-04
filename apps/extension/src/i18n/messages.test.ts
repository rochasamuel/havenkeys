import { describe, expect, it } from "vitest";
import { en } from "./en";
import { ptBR } from "./pt-BR";

type Leaf = [path: string, en: unknown, pt: unknown];

function leaves(a: unknown, b: unknown, path = ""): Leaf[] {
  if (typeof a === "object" && a !== null) {
    return Object.keys(a).flatMap((k) => leaves((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k], path ? `${path}.${k}` : k));
  }
  return [[path, a, b]];
}

/** Words that are the same in both languages. */
const SAME = new Set(["popup.pill.offline", "menu.documentLabels.cpf", "menu.documentLabels.rg"]);

describe("pt-BR messages", () => {
  const all = leaves(en, ptBR);

  it("translates every string", () => {
    const untranslated = all.filter(([path, e, p]) => typeof e === "string" && e === p && !SAME.has(path)).map(([path]) => path);
    expect(untranslated).toEqual([]);
  });

  it("keeps parameterised strings as functions that use their argument", () => {
    for (const [path, e, p] of all.filter(([, e]) => typeof e === "function")) {
      expect(typeof p, path).toBe("function");
      expect((p as (s: string) => string)("ARG"), path).toContain("ARG");
      expect((e as (s: string) => string)("ARG"), path).toContain("ARG");
    }
  });

  it("never translates the product name away", () => {
    for (const [path, e, p] of all) {
      if (typeof e === "string" && e.includes("HavenKeys")) expect(p, path).toContain("HavenKeys");
    }
  });
});

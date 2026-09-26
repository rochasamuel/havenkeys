// The manifest's __MSG_*__ placeholders must resolve in every locale the
// extension ships, or the browser shows the raw placeholder (or refuses to
// load the extension).
import { describe, expect, it } from "vitest";
import base from "../../manifest/base.json";
import en from "../../manifest/_locales/en/messages.json";
import ptBR from "../../manifest/_locales/pt_BR/messages.json";

const placeholders = [...JSON.stringify(base).matchAll(/__MSG_(\w+)__/g)].map((m) => m[1] as string);

describe("manifest locales", () => {
  it("uses placeholders with an English default", () => {
    expect(base.default_locale).toBe("en");
    expect(placeholders.sort()).toEqual(["extDescription", "extName"]);
  });

  for (const [name, messages] of [["en", en], ["pt_BR", ptBR]] as const) {
    it(`${name} defines every placeholder`, () => {
      const table = messages as Record<string, { message?: unknown }>;
      for (const key of placeholders) expect(typeof table[key]?.message, key).toBe("string");
      expect(Object.keys(messages).sort()).toEqual([...placeholders].sort());
    });
  }

  it("keeps the product name untranslated", () => {
    expect(en.extName.message).toBe("HavenKeys");
    expect(ptBR.extName.message).toBe("HavenKeys");
  });
});

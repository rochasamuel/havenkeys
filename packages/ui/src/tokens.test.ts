// tokens.css is checked in, so it can be read in a diff and loaded by the
// extension without a build step. That only works if it is never edited by
// hand and never falls behind tokens.ts, which is what these tests enforce.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { AUTO_CSS_PATH, TOKENS_CSS_PATH, generateAutoCss, generateTokensCss } from "./generate.ts";
import { THEME_ORDER, themes, type ThemeName } from "./tokens.ts";

const BASE_CSS_PATH = fileURLToPath(new URL("./base.css", import.meta.url));

/** The selector each theme is emitted under, for finding its block again. */
const THEME_SELECTORS: Readonly<Record<ThemeName, string>> = {
  dark: ':root[data-theme="dark"]',
  light: ':root[data-theme="light"]',
  systemLight: ':root[data-theme="system"]',
};

/** The text from `selector` to the `}` that closes its rule. */
function ruleBlock(css: string, selector: string): string {
  const start = css.indexOf(selector);
  if (start < 0) throw new Error(`selector not emitted: ${selector}`);

  let depth = 0;
  for (let i = css.indexOf("{", start); i < css.length; i += 1) {
    if (css[i] === "{") depth += 1;
    else if (css[i] === "}") {
      depth -= 1;
      if (depth === 0) return css.slice(start, i + 1);
    }
  }
  throw new Error(`unterminated rule: ${selector}`);
}

describe("tokens.css", () => {
  it("is exactly what generate.ts produces", () => {
    const checkedIn = readFileSync(TOKENS_CSS_PATH);
    const regenerated = Buffer.from(generateTokensCss(), "utf8");

    // Compare the text first: on a mismatch vitest shows which line moved,
    // which is the whole reason to look at this test.
    expect(regenerated.toString("utf8")).toBe(checkedIn.toString("utf8"));
    expect(regenerated.equals(checkedIn)).toBe(true);
  });

  it("defines every colour token in all three themes", () => {
    const names = Object.keys(themes.dark.colors);
    expect(names.length).toBeGreaterThan(0);

    // No theme may carry a colour the others lack: a token that exists only
    // in dark silently falls back to the dark value on paper, which is the
    // kind of thing nobody notices until it is shipped.
    for (const theme of THEME_ORDER) {
      expect(Object.keys(themes[theme].colors).sort()).toEqual([...names].sort());
    }

    // And the emitted CSS must agree, since that is what actually ships.
    const css = generateTokensCss();
    const expected = names.map((name) => `--${name}`).sort();
    for (const theme of THEME_ORDER) {
      const block = ruleBlock(css, THEME_SELECTORS[theme]);
      const declared = [...block.matchAll(/^\s*(--[a-z0-9-]+):/gmu)].map((m) => m[1]);
      expect(declared.sort(), `wrong colours under ${THEME_SELECTORS[theme]}`).toEqual(expected);
      expect(block).toContain(`color-scheme: ${themes[theme].colorScheme};`);
    }
  });

  it("only lets base.css reference tokens that exist", () => {
    const declared = new Set(
      [...generateTokensCss().matchAll(/^\s*(--[a-z0-9-]+):/gmu)].map((m) => m[1]),
    );
    const used = new Set(
      [...readFileSync(BASE_CSS_PATH, "utf8").matchAll(/var\((--[a-z0-9-]+)\)/gu)].map(
        (m) => m[1],
      ),
    );

    expect(used.size).toBeGreaterThan(0);
    expect([...used].filter((name) => !declared.has(name))).toEqual([]);
  });
});

describe("tokens-auto.css", () => {
  it("matches what generate.ts produces", () => {
    const checkedIn = readFileSync(AUTO_CSS_PATH, "utf8");
    expect(generateAutoCss()).toBe(checkedIn);
  });

  it("needs no theme attribute, because the extension has none to set", () => {
    const css = generateAutoCss();
    expect(css).not.toContain("data-theme");
    expect(css).toContain("@media (prefers-color-scheme: light)");
  });

  it("declares the same custom properties as tokens.css", () => {
    const names = (css: string) =>
      new Set(Array.from(css.matchAll(/^\s*(--[a-z0-9-]+):/gm), (m) => m[1]));
    expect(names(generateAutoCss())).toEqual(names(generateTokensCss()));
  });
});

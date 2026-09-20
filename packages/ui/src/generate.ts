// Turns src/tokens.ts into src/tokens.css.
//
// The CSS is checked in rather than built on demand because the extension
// loads it as a static file inside a frame, with no bundler in the path, and
// because a generated file in the tree is reviewable in a diff. src/tokens.
// test.ts regenerates and compares byte for byte, so the two cannot drift.
//
// Run it with: pnpm --filter @havenkeys/ui generate

import { writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  duration,
  easing,
  elevation,
  fontFamily,
  fontSize,
  fontWeight,
  layers,
  letterSpacing,
  lineHeight,
  radii,
  spacing,
  themes,
  type Css,
  type Theme,
} from "./tokens.ts";

const HEADER = `/* Generated file — do not edit by hand.
   Written by packages/ui/src/generate.ts from packages/ui/src/tokens.ts.
   Run \`pnpm --filter @havenkeys/ui generate\` after changing a token;
   src/tokens.test.ts fails if this file and tokens.ts disagree.

   Custom properties only. Loading this sets no colours on any element: the
   consumer decides what uses them. */`;

/** The theme-independent scales, in the order they are written out. */
const SCALE_GROUPS: ReadonlyArray<readonly [string, Readonly<Record<string, Css>>]> = [
  ["font stacks", fontFamily],
  ["type sizes", fontSize],
  ["type weights", fontWeight],
  ["line heights", lineHeight],
  ["letter spacing", letterSpacing],
  ["spacing", spacing],
  ["radii", radii],
  ["elevation", elevation],
  ["motion", { ...duration, ...easing }],
  ["layers", layers],
];

function declarations(record: Readonly<Record<string, Css>>, indent: string): string[] {
  return Object.entries(record).map(([name, value]) => `${indent}--${name}: ${value};`);
}

function themeBlock(selector: string, theme: Theme, indent: string): string {
  return [
    `${indent}${selector} {`,
    ...declarations(theme.colors, `${indent}  `),
    // Native controls and scrollbars read this, not the custom properties.
    `${indent}  color-scheme: ${theme.colorScheme};`,
    `${indent}}`,
  ].join("\n");
}

/** The exact contents of src/tokens.css. */
export function generateTokensCss(): string {
  const parts: string[] = [HEADER, ""];

  parts.push(themeBlock(':root,\n:root[data-theme="dark"]', themes.dark, ""), "");
  parts.push(themeBlock(':root[data-theme="light"]', themes.light, ""), "");

  parts.push(
    '/* [data-theme="system"] follows the OS. Dark is already the default above,',
    "   so only the light case needs saying. */",
    "@media (prefers-color-scheme: light) {",
    themeBlock(':root[data-theme="system"]', themes.systemLight, "  "),
    "}",
    "",
  );

  parts.push("/* Scales. None of these change with the theme. */", ":root {");
  SCALE_GROUPS.forEach(([label, record], index) => {
    if (index > 0) parts.push("");
    parts.push(`  /* ${label} */`, ...declarations(record, "  "));
  });
  parts.push("}");

  return `${parts.join("\n")}\n`;
}

export const TOKENS_CSS_PATH = resolve(dirname(fileURLToPath(import.meta.url)), "tokens.css");

// Only write when run as a script; importing this module must have no effect,
// or the test that compares against the checked-in file would rewrite it first.
const entry = process.argv[1];
if (entry !== undefined && resolve(entry) === fileURLToPath(import.meta.url)) {
  writeFileSync(TOKENS_CSS_PATH, generateTokensCss(), "utf8");
  process.stdout.write(`wrote ${TOKENS_CSS_PATH}\n`);
}

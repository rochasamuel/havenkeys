import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { generate, normalize } from "./generate.mjs";

/** Runs `generate` on `words` in a temp dir; returns the outputs' paths. */
function run(words, check = false, dir = mkdtempSync(join(tmpdir(), "words-"))) {
  const wordsPath = join(dir, "words.json");
  writeFileSync(wordsPath, JSON.stringify(words));
  const out = { ts: join(dir, "words.generated.ts"), kotlin: join(dir, "SharedWords.kt") };
  return { dir, out, ...generate({ check, wordsPath, out }) };
}

test("rejects a word that is not normalized", () => {
  assert.throws(() => run({ lists: { X: { words: ["Usuário"] } } }), /not normalized: X: Usuário/);
  assert.throws(() => run({ lists: { X: { words: ["e-mail"] } } }), /not normalized: X: e-mail/);
});

test("raw lists skip the check", () => {
  assert.doesNotThrow(() => run({ lists: { X: { raw: true, words: ["given-name"] } } }));
});

test("per-platform entries reach only their platform", () => {
  const { out } = run({ lists: { A: { extension: ["a"] } }, numbers: { N: { android: 4 } } });
  const ts = readFileSync(out.ts, "utf8");
  const kt = readFileSync(out.kotlin, "utf8");
  assert.match(ts, /export const A/);
  assert.doesNotMatch(kt, /val A/);
  assert.match(kt, /const val N = 4/);
  assert.doesNotMatch(ts, /export const N/);
});

test("check reports stale output", () => {
  const words = { lists: { A: { words: ["a"] } } };
  const { dir, out } = run(words);
  assert.deepEqual(run(words, true, dir).stale, []);
  writeFileSync(out.ts, readFileSync(out.ts, "utf8") + "// edited\n");
  assert.deepEqual(run(words, true, dir).stale, [out.ts]);
});

test("normalize matches the shared vectors", () => {
  const vectors = JSON.parse(readFileSync(new URL("./fixtures/normalize.json", import.meta.url), "utf8"));
  for (const { in: raw, out } of vectors) assert.equal(normalize(raw), out, raw);
});

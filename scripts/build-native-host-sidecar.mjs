#!/usr/bin/env node
// Builds havenkeys-native-host in release mode and places it where Tauri
// looks for the `externalBin` declared in
// apps/desktop/src-tauri/tauri.bundle.conf.json:
//   apps/desktop/src-tauri/binaries/havenkeys-native-host-<target>[.exe]
//
//   node scripts/build-native-host-sidecar.mjs [--target <rust target triple>]
//
// Without --target it builds for this machine. `universal-apple-darwin`
// builds both macOS architectures and joins them with lipo, as Tauri's
// universal macOS build expects. Only installer builds need this: the
// sidecar is declared in a separate config file, so `cargo test`, clippy and
// `tauri dev` work without it.

import { execFileSync } from "node:child_process";
import { copyFileSync, chmodSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const run = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: "inherit" });

const i = process.argv.indexOf("--target");
const target =
  i > 0
    ? process.argv[i + 1]
    : /^host: (\S+)$/m.exec(execFileSync("rustc", ["-vV"], { encoding: "utf8" }))[1];
if (!target || !/^[a-z0-9_-]+$/.test(target)) {
  throw new Error(`invalid target triple: ${target}`);
}

const exe = target.includes("windows") ? ".exe" : "";
const built = (t) => join(root, "target", t, "release", `havenkeys-native-host${exe}`);
const build = (t) => run("cargo", ["build", "--release", "--locked", "-p", "havenkeys-native-host", "--target", t]);

const outDir = join(root, "apps/desktop/src-tauri/binaries");
const out = join(outDir, `havenkeys-native-host-${target}${exe}`);
mkdirSync(outDir, { recursive: true });

if (target === "universal-apple-darwin") {
  const archs = ["aarch64-apple-darwin", "x86_64-apple-darwin"];
  for (const t of archs) build(t);
  run("lipo", ["-create", "-output", out, ...archs.map(built)]);
} else {
  build(target);
  copyFileSync(built(target), out);
}
if (!exe) chmodSync(out, 0o755);
console.log(`native host sidecar: ${out}`);

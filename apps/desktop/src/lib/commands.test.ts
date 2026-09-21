// The command surface is declared in four places that must agree:
//
//   src-tauri/build.rs             generates one permission per command
//   src-tauri/src/lib.rs           registers the handlers
//   src-tauri/capabilities/main.json  grants the window exactly these
//   src/lib/api.ts                 what the UI can actually call
//
// A command missing from the capability is denied by Tauri before it reaches
// Rust, and the renderer sees a rejection it cannot parse — "Something went
// wrong", with nothing in any log to say which call it was. That shipped
// once; this test is why it cannot ship again.
//
// It reads the files as text rather than importing anything: the point is to
// check what the build will see, including on a machine with no Rust
// toolchain.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const read = (relative: string) =>
  readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");

/** Everything a regex captured in group 1, with the misses dropped. */
function captured(text: string, pattern: RegExp): string[] {
  return Array.from(text.matchAll(pattern), (m) => m[1]).filter(
    (name): name is string => name !== undefined,
  );
}

/** The one block a parser needs, or a failure that names which file moved. */
function block(source: string, pattern: RegExp, what: string): string {
  const found = pattern.exec(source);
  expect(found?.[1], `${what} no longer reads the way this test parses it`).toBeTruthy();
  return found?.[1] ?? "";
}

/** `const COMMANDS: &[&str] = &[ "a", "b" ];` → ["a", "b"] */
function declaredInBuildRs(): string[] {
  const source = read("../../src-tauri/build.rs");
  const list = block(source, /const COMMANDS: &\[&str\] = &\[([\s\S]*?)\];/, "build.rs");
  return captured(list, /"([a-z0-9_]+)"/g);
}

/** The `tauri::generate_handler![...]` list, minus its module paths. */
function registeredInLibRs(): string[] {
  const source = read("../../src-tauri/src/lib.rs");
  const list = block(source, /generate_handler!\[([\s\S]*?)\]/, "lib.rs");
  return captured(list, /(?:^|\s)(?:\w+::)?(\w+),/g);
}

/** `allow-create-item` → `create_item`, ignoring Tauri's own `core:` grants. */
function grantedInCapability(): string[] {
  const capability = JSON.parse(read("../../src-tauri/capabilities/main.json")) as {
    permissions: string[];
  };
  return capability.permissions
    .filter((p) => p.startsWith("allow-"))
    .map((p) => p.slice("allow-".length).replaceAll("-", "_"));
}

/** Every `call<T>("name")` the UI makes. */
function calledFromApi(): string[] {
  return captured(read("./api.ts"), /call<[^>]*>\(\s*"([a-z0-9_]+)"/g);
}

describe("the Tauri command surface", () => {
  it("is granted to the window exactly as it is registered", () => {
    expect([...grantedInCapability()].sort()).toEqual([...registeredInLibRs()].sort());
  });

  it("has a generated permission for every registered command", () => {
    expect([...declaredInBuildRs()].sort()).toEqual([...registeredInLibRs()].sort());
  });

  it("registers every command the UI calls", () => {
    const registered = new Set(registeredInLibRs());
    const missing = [...new Set(calledFromApi())].filter((name) => !registered.has(name));
    expect(missing, "api.ts calls commands that do not exist").toEqual([]);
  });

  it("finds the lists at all", () => {
    // Guards the regexes above: if a refactor renames the blocks, these
    // parsers would quietly return nothing and every check would pass.
    expect(declaredInBuildRs().length).toBeGreaterThan(15);
    expect(registeredInLibRs().length).toBeGreaterThan(15);
    expect(grantedInCapability().length).toBeGreaterThan(15);
    expect(calledFromApi().length).toBeGreaterThan(15);
  });
});

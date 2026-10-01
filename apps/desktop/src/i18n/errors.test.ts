import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { ApiError, toApiError } from "../lib/api";
import { en } from "./en";
import { errorMessage } from "./errors";
import { ptBR } from "./pt-BR";

describe("errorMessage", () => {
  it("translates a known code", () => {
    const e = new ApiError("locked", "The vault is locked.");
    expect(errorMessage(e, en)).toBe("The vault is locked.");
    expect(errorMessage(e, ptBR)).toBe("O cofre está bloqueado.");
  });

  it("shows Rust's message for a code it does not know", () => {
    const e = new ApiError("from_a_newer_core", "Something new happened.");
    expect(errorMessage(e, ptBR)).toBe("Something new happened.");
  });

  it("shows Rust's message where the text varies", () => {
    const e = new ApiError("vault_unreadable", "This vault … Your file is in /home/x/.local/share/havenkeys. …");
    expect(errorMessage(e, en)).toBe(e.message);
    expect(errorMessage(e, ptBR)).toBe(e.message);
  });

  it("translates the details of invalid input it knows", () => {
    const e = new ApiError("invalid_input", "Invalid input: title is required.");
    expect(errorMessage(e, en)).toBe("Invalid input: title is required.");
    expect(errorMessage(e, ptBR)).toBe("O título é obrigatório.");
    const other = new ApiError("invalid_input", "Invalid input: blob context.");
    expect(errorMessage(other, ptBR)).toBe("Invalid input: blob context.");
  });

  it("never passes through text that did not come from the core", () => {
    expect(errorMessage(new Error("hunter2"), en)).toBe(en.errors.generic);
    expect(errorMessage("hunter2", ptBR)).toBe(ptBR.errors.generic);
    expect(errorMessage(undefined, ptBR, "Não foi possível copiar.")).toBe("Não foi possível copiar.");
  });

  it("does not treat inherited properties as codes", () => {
    const e = new ApiError("toString", "Internal.");
    expect(errorMessage(e, ptBR)).toBe("Internal.");
  });
});

// The table must follow the Rust sources, read as text like commands.test.ts
// does: every code Rust can send is listed, and the English is Rust's own.

const root = fileURLToPath(new URL("../../../../", import.meta.url));

function rustFiles(dir: string): string[] {
  return readdirSync(join(root, dir), { recursive: true, encoding: "utf8" })
    .filter((f) => f.endsWith(".rs"))
    .map((f) => readFileSync(join(root, dir, f), "utf8"));
}

/** code → message(s) as Rust writes them; `null` when built with format!. */
function rustMessages(): Map<string, Set<string | null>> {
  const out = new Map<string, Set<string | null>>();
  const add = (code: string, message: string | null) => {
    out.set(code, (out.get(code) ?? new Set()).add(message));
  };
  const core = readFileSync(join(root, "crates/havenkeys-core/src/error.rs"), "utf8");
  const byVariant = new Map(
    Array.from(core.matchAll(/#\[error\("([^"]*)"\)\]\s*(\w+)/g), (m) => [m[2] ?? "", m[1] ?? ""]),
  );
  for (const m of core.matchAll(/Error::(\w+)(?:\(_\))? => "(\w+)"/g)) {
    const message = byVariant.get(m[1] ?? "");
    add(m[2] ?? "", message?.includes("{") ? null : (message ?? null));
  }
  for (const source of rustFiles("apps/desktop/src-tauri/src")) {
    for (const m of source.matchAll(/code: "(\w+)",\s*message: (?:"([^"]*)"|format!)/g)) {
      add(m[1] ?? "", m[2] ?? null);
    }
  }
  // The UI's own IPC wrapper sends "internal" too, with its own message.
  add("internal", toApiError(null).message);
  return out;
}

describe("the error table", () => {
  const rust = rustMessages();

  it("finds the Rust codes at all", () => {
    expect(rust.size).toBeGreaterThan(25);
    expect(rust.has("locked")).toBe(true);
    expect(rust.has("sign_in_failed")).toBe(true);
  });

  it("lists exactly the codes Rust sends", () => {
    expect(Object.keys(en.errors.codes).sort()).toEqual([...rust.keys()].sort());
  });

  it("keeps the English identical to Rust's, or defers to it where it varies", () => {
    for (const [code, messages] of rust) {
      const text = en.errors.codes[code as keyof typeof en.errors.codes];
      if (messages.size === 1 && !messages.has(null)) {
        expect(text, code).toBe([...messages][0]);
      } else {
        expect(text, `${code} varies in Rust`).toBeNull();
      }
    }
  });

  it("defers to Rust in Portuguese wherever the English does", () => {
    // `null` in English means Rust's text varies in meaning; a fixed
    // translation would say something Rust did not. The one exception is
    // "internal", whose variants all mean "something went wrong".
    const translatedAnyway = new Set(["internal"]);
    const deferred = Object.entries(en.errors.codes)
      .filter(([code, text]) => text === null && !translatedAnyway.has(code))
      .map(([code]) => code);
    expect(deferred.length).toBeGreaterThan(0);
    for (const code of deferred) {
      expect(ptBR.errors.codes[code as keyof typeof ptBR.errors.codes], code).toBeNull();
    }
    const e = new ApiError("sync_failed", "The server did not acknowledge that item.");
    expect(errorMessage(e, ptBR)).toBe(e.message);
  });

  it("translates only invalid-input details Rust actually has", () => {
    const sources = [...rustFiles("crates"), ...rustFiles("apps/desktop/src-tauri/src")].join("\n");
    // Details are literals in `InvalidInput("…")` or `const …_MSG: &str = "…"` handed to it.
    // `clean_value` takes its detail as a bare literal, always "… contains control characters".
    const details = new Set([
      ...Array.from(sources.matchAll(/"([^"]* is too long or contains control characters)"/g), (m) => m[1]),
      ...Array.from(sources.matchAll(/InvalidInput\("([^"]*)"\)/g), (m) => m[1]),
      ...Array.from(sources.matchAll(/const [A-Z_]+_MSG: &str = "([^"]*)";/g), (m) => m[1]),
    ]);
    const unknown = Object.keys(ptBR.errors.invalidInput).filter((d) => !details.has(d));
    expect(unknown).toEqual([]);
  });

  it("keeps Rust's keychain warning word for word", () => {
    const removal = readFileSync(join(root, "apps/desktop/src-tauri/src/removal.rs"), "utf8");
    const rustText = /const KEYCHAIN_NOT_CLEARED: &str = "([^"]*)";/.exec(removal)?.[1];
    expect(rustText?.replaceAll("\\u{201c}", "“").replaceAll("\\u{201d}", "”")).toBe(en.app.keychainNotCleared);
  });
});

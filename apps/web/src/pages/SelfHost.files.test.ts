// @vitest-environment node
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { ptBR } from "../i18n/pt-BR";

const bundleDir = fileURLToPath(new URL("../../../../deploy/compose/", import.meta.url));

describe("Self-host bundle download step", () => {
  it("lists only files that exist in deploy/compose", () => {
    const m = en.selfHost.steps[1]?.code.match(/for f in ([^;]+); do/);
    expect(m).not.toBeNull();
    const files = (m?.[1] ?? "").trim().split(/\s+/);
    expect(files.length).toBeGreaterThan(0);
    for (const f of files) expect(existsSync(bundleDir + f), f).toBe(true);
  });
  it("has the same command in pt-BR", () => {
    expect(ptBR.selfHost.steps[1]?.code).toBe(en.selfHost.steps[1]?.code);
  });
});

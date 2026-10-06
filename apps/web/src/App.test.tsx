import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { Site } from "./App";
import { en } from "./i18n/en";
import { ptBR } from "./i18n/pt-BR";

function html(path: string): string {
  return renderToString(
    <MemoryRouter initialEntries={[path]}>
      <Site />
    </MemoryRouter>,
  );
}

const text = (s: string) => s.replace(/&#x27;|’/g, "'");

describe("routes", () => {
  it("renders the Developers page in both languages", () => {
    expect(text(html("/developers"))).not.toContain(text(en.notFound.title));
    expect(text(html("/pt-br/developers"))).not.toContain(text(ptBR.notFound.title));
  });
  it("still renders every existing page", () => {
    for (const p of ["/", "/download", "/security", "/privacy", "/terms", "/delete-account"]) {
      expect(text(html(p)), p).not.toContain(text(en.notFound.title));
      expect(text(html(`/pt-br${p === "/" ? "" : p}`)), p).not.toContain(text(ptBR.notFound.title));
    }
  });
});

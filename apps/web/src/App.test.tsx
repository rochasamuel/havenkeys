import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
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
  it("shows the technical content on Developers and the plain copy on Home and Security", () => {
    expect(text(html("/developers"))).toContain(text(en.developers.heroTitle));
    expect(text(html("/pt-br/developers"))).toContain(text(ptBR.developers.heroTitle));
    expect(text(html("/security"))).toContain(text(en.security.heroTitle));
    expect(text(html("/"))).toContain(text(en.home.trustTitle));
  });
  it("keeps the old Home anchors", () => {
    const home = html("/");
    expect(home).toContain('id="journey"');
    expect(home).toContain('id="browser"');
  });
  it("links Home to the signup page", () => {
    expect(html("/")).toContain('href="/signup"');
  });
  it("shows the Beta tag in the nav on every page", () => {
    for (const path of ["/", "/download", "/signup", "/pricing", "/pt-br"]) {
      expect(html(path), path).toContain('class="tag tag--beta"');
    }
  });
  it("keeps the Beta tag beside the wordmark and names it in the link label", () => {
    const h = html("/");
    expect(h).toContain('class="nav__wordmark"');
    expect(h).toContain('class="tag tag--beta"');
    expect(h).toContain(`aria-label="${en.nav.homeAria}"`);
    expect(en.nav.homeAria.toLowerCase()).toContain("beta");
    expect(ptBR.nav.homeAria.toLowerCase()).toContain("beta");
  });
  it("never hides the brand's descendants with a blanket selector", () => {
    const css = readFileSync(fileURLToPath(new URL("./styles/global.css", import.meta.url)), "utf8");
    expect(css).not.toMatch(/\.nav__brand\s+span/);
  });
  it("still renders every existing page", () => {
    for (const p of ["/", "/download", "/security", "/self-host", "/privacy", "/terms", "/delete-account"]) {
      expect(text(html(p)), p).not.toContain(text(en.notFound.title));
      expect(text(html(`/pt-br${p === "/" ? "" : p}`)), p).not.toContain(text(ptBR.notFound.title));
    }
  });
});

import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { SelfHost } from "./SelfHost";

const html = () => renderToString(<MemoryRouter><SelfHost /></MemoryRouter>);

describe("SelfHost", () => {
  it("shows the four Docker steps and the bundle download", () => {
    const h = html();
    expect(en.selfHost.steps).toHaveLength(4);
    expect(h).toContain("deploy/compose/$f");
    expect(h).toContain("./setup.sh");
    expect(h).toContain("admin new-account");
  });
  it("shows no Railway button until a template exists", () => {
    const h = html();
    expect(h).not.toContain("railway.com/deploy");
    expect(h.replace(/&#x27;|’/g, "'")).toContain(en.selfHost.railwaySoon.replace(/’/g, "'"));
  });
});

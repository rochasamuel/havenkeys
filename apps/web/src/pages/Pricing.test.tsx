import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { Pricing } from "./Pricing";

describe("Pricing", () => {
  it("shows the Personal plan, the trial and what a frozen account keeps", () => {
    const h = renderToString(
      <MemoryRouter>
        <Pricing />
      </MemoryRouter>,
    );
    expect(h).toContain(en.pricing.plan);
    expect(h).toContain(en.pricing.trial);
    expect(h).toContain(en.pricing.priceSoon);
    expect(h).toContain(en.pricing.keepsTitle);
    expect(h).toContain(en.pricing.pausesTitle);
    for (const line of [...en.pricing.keeps, ...en.pricing.pauses]) expect(h).toContain(line);
    expect(h).toContain('href="mailto:samuelsilv.rocha@gmail.com"');
    expect(h).toContain('href="/signup"');
    expect(h).toContain(en.common.betaNotice);
  });
});

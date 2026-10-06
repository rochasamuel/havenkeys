import { describe, expect, it } from "vitest";
import { INVITE_EMAIL, inviteHref, RAILWAY_TEMPLATE_URL } from "./links";

describe("links", () => {
  it("builds the invite mailto with an encoded subject", () => {
    expect(INVITE_EMAIL).toBe("invite@havenkeys.net");
    expect(inviteHref("Pedido de convite HavenKeys")).toBe("mailto:invite@havenkeys.net?subject=Pedido%20de%20convite%20HavenKeys");
  });
  it("has no Railway template until the owner publishes one", () => {
    expect(RAILWAY_TEMPLATE_URL).toBe("");
  });
});

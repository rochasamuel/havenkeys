import { describe, expect, it } from "vitest";
import { parseBackgroundMessage, parseContentRequest, parseIdentityRolesReply, parseInlineRequest } from "./inline";

const TOKEN = "a".repeat(32);

describe("identity messages", () => {
  it("cs_open_menu carries the form's roles", () => {
    expect(parseContentRequest({ type: "cs_open_menu", kind: "identity", roles: ["fullName", "cpf"] })).toEqual({
      type: "cs_open_menu",
      kind: "identity",
      roles: ["fullName", "cpf"],
    });
    expect(parseContentRequest({ type: "cs_open_menu", kind: "login", roles: ["email"] })).not.toBeNull();
    for (const bad of [
      { type: "cs_open_menu", kind: "identity" },
      { type: "cs_open_menu", kind: "identity", roles: [] },
      { type: "cs_open_menu", kind: "identity", roles: ["password"] },
      { type: "cs_open_menu", kind: "identity", roles: ["city", "city"] },
      { type: "cs_open_menu", kind: "otp", roles: ["city"] },
      { type: "cs_open_menu", kind: "identity", roles: Array(41).fill("city") },
    ]) {
      expect(parseContentRequest(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("menu requests for the identity", () => {
    expect(parseInlineRequest({ type: "menu_pick_identity", token: TOKEN, documents: true })).toEqual({
      type: "menu_pick_identity",
      token: TOKEN,
      documents: true,
    });
    expect(parseInlineRequest({ type: "menu_open_identity", token: TOKEN })).not.toBeNull();
    expect(parseInlineRequest({ type: "menu_pick_identity", token: TOKEN })).toBeNull();
    expect(parseInlineRequest({ type: "menu_pick_identity", token: TOKEN, documents: "yes" })).toBeNull();
  });

  it("identity fills and role scans from the background", () => {
    const fill = {
      type: "bg_fill",
      origin: "https://shop.com",
      token: TOKEN,
      fill: { kind: "identity", values: [{ role: "city", value: "Brasília" }] },
      submit: false,
      totp: false,
    };
    expect(parseBackgroundMessage(fill)).toEqual(fill);
    expect(parseBackgroundMessage({ ...fill, submit: true })).toBeNull(); // identity never submits
    expect(parseBackgroundMessage({ ...fill, fill: { kind: "identity", values: [{ role: "x", value: "y" }] } })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_identity_roles" })).toEqual({ type: "bg_identity_roles" });
    expect(parseIdentityRolesReply({ roles: ["cpf", "city"] })).toEqual(["cpf", "city"]);
    expect(parseIdentityRolesReply({ roles: ["nope"] })).toEqual([]);
    expect(parseIdentityRolesReply(undefined)).toEqual([]);
  });
});

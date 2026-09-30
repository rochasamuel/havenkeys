import { describe, expect, it } from "vitest";
import { parseBackgroundMessage, parseContentRequest, parseHostReply, parseInlineRequest } from "./inline";

const T = "a".repeat(32);
const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const anchor = { top: 10, left: 20, width: 200, height: 30 };
const card = { number: "4111111111111111", expiry: "2033-04", verificationNumber: "123", cardholderName: "Samuel" };

describe("card messages", () => {
  it("accepts exact card content requests", () => {
    expect(parseContentRequest({ type: "cs_open_menu", kind: "card", cardRoles: ["number", "expiryMonth"] })).toEqual({
      type: "cs_open_menu",
      kind: "card",
      cardRoles: ["number", "expiryMonth"],
    });
    expect(parseContentRequest({ type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor })).toMatchObject({ anchor });
    expect(parseContentRequest({ type: "cs_card_fields", scan: T, roles: ["verificationNumber"] })).not.toBeNull();
    expect(parseContentRequest({ type: "cs_card_submit", card })).toEqual({ type: "cs_card_submit", card });
    expect(parseContentRequest({ type: "cs_card_submit", card: { ...card, verificationNumber: null, cardholderName: null } })).not.toBeNull();
  });

  it("rejects malformed card content requests", () => {
    for (const bad of [
      { type: "cs_open_menu", kind: "card" },
      { type: "cs_open_menu", kind: "card", cardRoles: [] },
      { type: "cs_open_menu", kind: "card", cardRoles: ["number", "number"] },
      { type: "cs_open_menu", kind: "card", cardRoles: ["password"] },
      { type: "cs_open_menu", kind: "card", cardRoles: ["number"], roles: ["fullName"] },
      { type: "cs_open_menu", kind: "login", cardRoles: ["number"] },
      { type: "cs_open_menu", kind: "login", anchor },
      { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor: { ...anchor, width: -1 } },
      { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor: { ...anchor, top: Infinity } },
      { type: "cs_card_fields", scan: "x", roles: ["number"] },
      { type: "cs_card_fields", scan: T, roles: [] },
      { type: "cs_card_submit", card: { ...card, number: "4111" } },
      { type: "cs_card_submit", card: { ...card, expiry: "04/33" } },
      { type: "cs_card_submit", card: { ...card, verificationNumber: "12" } },
      { type: "cs_card_submit", card: { ...card, cardholderName: "" } },
      { type: "cs_card_submit", card: { ...card, extra: 1 } },
    ]) {
      expect(parseContentRequest(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("accepts card background messages; a card fill never submits", () => {
    const fill = { kind: "card", values: [{ role: "number", value: "4111111111111111" }] };
    expect(parseBackgroundMessage({ type: "bg_fill", origin: "https://a.com", token: null, fill, submit: false, totp: false })).not.toBeNull();
    expect(parseBackgroundMessage({ type: "bg_fill", origin: "https://a.com", token: null, fill, submit: true, totp: false })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_fill", origin: "https://a.com", token: null, fill: { kind: "card", values: [{ role: "number", value: "" }] }, submit: false, totp: false })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_card_scan", scan: T })).toEqual({ type: "bg_card_scan", scan: T });
    expect(parseBackgroundMessage({ type: "bg_host_menu", token: T, frameId: 3, anchor, rows: 2 })).not.toBeNull();
    expect(parseBackgroundMessage({ type: "bg_host_menu", token: T, frameId: 0, anchor, rows: 2 })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_host_menu", token: T, frameId: 3, anchor, rows: 9 })).toBeNull();
  });

  it("accepts menu_pick_card and host replies exactly", () => {
    expect(parseInlineRequest({ type: "menu_pick_card", token: T, itemId: ID })).toEqual({ type: "menu_pick_card", token: T, itemId: ID });
    expect(parseInlineRequest({ type: "menu_pick_card", token: T, itemId: "x" })).toBeNull();
    expect(parseHostReply({ ok: true })).toBe(true);
    expect(parseHostReply({ ok: false })).toBe(false);
    expect(parseHostReply(undefined)).toBe(false);
  });
});

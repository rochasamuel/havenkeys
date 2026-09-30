// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { CardValue } from "@havenkeys/protocol";
import type { Env } from "./group";
import { cardGroupFor } from "./card";
import { cardRolesToFill, expiryFor, fillCard, matchCardOption, readCardSubmission } from "./card-fill";
import { markUserEdit } from "./fill";

let hidden = new Set<string>();
const env: Env = { isVisible: (el) => !el.hidden && !hidden.has(el.id), path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = <T extends HTMLElement = HTMLInputElement>(sel: string) => document.querySelector(sel) as T;
const input = (attrs: string) => {
  page(`<input id="x" ${attrs}>`);
  return $("#x");
};

const VALUES: CardValue[] = [
  { role: "cardholderName", value: "Samuel S Rocha" },
  { role: "cardholderGivenName", value: "Samuel" },
  { role: "cardholderFamilyName", value: "S Rocha" },
  { role: "number", value: "4111111111111111" },
  { role: "verificationNumber", value: "123" },
  { role: "expiryMonth", value: "4" },
  { role: "expiryYear", value: "2033" },
  { role: "brand", value: "visa" },
];

beforeEach(() => {
  page("");
  hidden = new Set();
});

describe("expiry in one field", () => {
  it("follows the placeholder, then maxlength, then MM/YY", () => {
    expect(expiryFor(input(`placeholder="MM/AA"`), "4", "2033")).toBe("04/33");
    expect(expiryFor(input(`placeholder="MM / YY"`), "4", "2033")).toBe("04 / 33");
    expect(expiryFor(input(`placeholder="MM/YYYY"`), "11", "2033")).toBe("11/2033");
    expect(expiryFor(input(`placeholder="mm-yy"`), "11", "2033")).toBe("11-33");
    expect(expiryFor(input(`placeholder="MMYY"`), "11", "2033")).toBe("1133");
    expect(expiryFor(input(`maxlength="4"`), "4", "2033")).toBe("0433");
    expect(expiryFor(input(`maxlength="7"`), "4", "2033")).toBe("04/2033");
    expect(expiryFor(input(``), "4", "2033")).toBe("04/33");
  });
});

describe("selects", () => {
  it("month selects by number, name and mixed text; never the placeholder", () => {
    page(`<select id="a"><option value="">Mês</option><option value="1">01</option><option value="4">04</option></select>
      <select id="b"><option>Month</option><option value="03">03 - Março</option><option value="04">04 - Abril</option></select>
      <select id="c"><option value="">--</option><option value="jan">January</option><option value="apr">April</option></select>`);
    expect(matchCardOption($("#a"), "expiryMonth", "4")).toBe(2);
    expect(matchCardOption($("#b"), "expiryMonth", "4")).toBe(2);
    expect(matchCardOption($("#c"), "expiryMonth", "4")).toBe(2);
    expect(matchCardOption($("#a"), "expiryMonth", "12")).toBe(-1);
  });
  it("year selects by 2 or 4 digits; brand selects by id, name or code", () => {
    page(`<select id="y"><option value="">Ano</option><option value="32">32</option><option value="33">33</option></select>
      <select id="b"><option value="">--</option><option value="MC">Master</option><option value="VI">Visa</option></select>`);
    expect(matchCardOption($("#y"), "expiryYear", "2033")).toBe(2);
    expect(matchCardOption($("#b"), "brand", "visa")).toBe(2);
    expect(matchCardOption($("#b"), "brand", "mastercard")).toBe(1);
    expect(matchCardOption($("#b"), "brand", "elo")).toBe(-1);
  });
});

describe("fillCard", () => {
  const FORM = `<form>
    <input id="num" name="numero_cartao" aria-label="Número do cartão" placeholder="0000 0000 0000 0000" maxlength="19">
    <input id="nome" name="nome_impresso" aria-label="Nome impresso no cartão">
    <input id="val" name="validade" placeholder="MM/AA" maxlength="5">
    <input id="cvv" type="password" name="cvv" maxlength="3">
  </form>`;
  const group = () => cardGroupFor($("#num"), env)!;

  it("fills each field in its shape and fires input and change", () => {
    page(FORM);
    const events: string[] = [];
    $("#val").addEventListener("input", () => events.push("input"));
    $("#val").addEventListener("change", () => events.push("change"));
    expect(fillCard(group(), VALUES, env)).toBe(4);
    expect($("#num").value).toBe("4111 1111 1111 1111");
    expect($("#nome").value).toBe("Samuel S Rocha");
    expect($("#val").value).toBe("04/33");
    expect($("#cvv").value).toBe("123");
    expect(events).toEqual(["input", "change"]);
    expect(document.body.innerHTML).not.toContain("4111");
  });

  it("leaves an Amex code out of a 3-character CVV field instead of cutting it", () => {
    page(FORM);
    fillCard(group(), VALUES.map((v) => (v.role === "verificationNumber" ? { ...v, value: "1234" } : v)), env);
    expect($("#cvv").value).toBe("");
  });

  it("fills month and year selects, padded text months and names split in two", () => {
    page(`<form><input id="num" autocomplete="cc-number"><input id="g" autocomplete="cc-given-name"><input id="f" autocomplete="cc-family-name">
      <input id="m" autocomplete="cc-exp-month" maxlength="2"><select id="y" autocomplete="cc-exp-year"><option value="">Ano</option><option>2033</option></select></form>`);
    fillCard(cardGroupFor($("#num"), env)!, VALUES, env);
    expect($("#g").value).toBe("Samuel");
    expect($("#f").value).toBe("S Rocha");
    expect($("#m").value).toBe("04");
    expect($<HTMLSelectElement>("#y").value).toBe("2033");
  });

  it("fills a split number slice by slice", () => {
    page(`<form><label for="c1">Card number</label><input id="c1" maxlength="4"><input id="c2" maxlength="4"><input id="c3" maxlength="4"><input id="c4" maxlength="4"></form>`);
    fillCard(cardGroupFor($("#c1"), env)!, VALUES, env);
    expect(["#c1", "#c2", "#c3", "#c4"].map((s) => $(s).value)).toEqual(["4111", "1111", "1111", "1111"]);
  });

  it("never overwrites what the user typed, but updates its own values", () => {
    page(FORM);
    $("#nome").value = "Typed";
    fillCard(group(), VALUES, env);
    expect($("#nome").value).toBe("Typed");
    expect(fillCard(group(), VALUES.map((v) => (v.role === "number" ? { ...v, value: "5555555555554444" } : v)), env)).toBeGreaterThan(0);
    expect($("#num").value).toBe("5555 5555 5555 4444");
  });

  it("skips a field hidden after the menu opened", () => {
    page(FORM);
    const g = group();
    hidden.add("cvv");
    fillCard(g, VALUES, env);
    expect($("#cvv").value).toBe("");
  });

  it("goes around an instance value setter (React's tracker) and still announces", () => {
    page(FORM);
    const el = $("#nome");
    let instanceSets = 0;
    Object.defineProperty(el, "value", {
      configurable: true,
      get: () => Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.get!.call(el),
      set: () => void instanceSets++,
    });
    fillCard(group(), VALUES, env);
    expect(instanceSets).toBe(0);
    expect(el.value).toBe("Samuel S Rocha");
  });

  it("asks only for the roles of fields it could write now", () => {
    page(FORM);
    $("#cvv").value = "999";
    expect(cardRolesToFill(group(), env)).toEqual(["number", "cardholderName", "expiryMonth", "expiryYear"]);
  });
});

describe("readCardSubmission", () => {
  const FORM = `<form><input id="num" autocomplete="cc-number"><input id="exp" autocomplete="cc-exp"><input id="cvv" autocomplete="cc-csc"><input id="n" autocomplete="cc-name"></form>`;
  const typed = (sel: string, v: string) => {
    $(sel).value = v;
    markUserEdit($(sel));
  };

  it("reads a card the user typed", () => {
    page(FORM);
    typed("#num", "4111 1111 1111 1111");
    typed("#exp", "04/33");
    typed("#cvv", "123");
    typed("#n", " Samuel Rocha ");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)).toEqual({
      number: "4111111111111111",
      expiry: "2033-04",
      verificationNumber: "123",
      cardholderName: "Samuel Rocha",
    });
  });

  it("offers to save a masked number the user typed", () => {
    page(FORM);
    typed("#num", "41111111111111111".slice(0, 16));
    // The site's mask reformats after the user's input event.
    $("#num").value = "4111 1111 1111 1111";
    typed("#exp", "0433");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)?.number).toBe("4111111111111111");
  });

  it("ignores numbers a page script wrote, failing numbers and missing expiries", () => {
    page(FORM);
    $("#num").value = "4111111111111111"; // no user edit
    typed("#exp", "04/33");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)).toBeNull();
    typed("#num", "4111111111111112");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)).toBeNull();
    typed("#num", "4111111111111111");
    typed("#exp", "");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)).toBeNull();
  });
});

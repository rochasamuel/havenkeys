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

describe("fix round 1", () => {
  const V2: CardValue[] = [
    { role: "cardholderName", value: "Ana Lima" },
    { role: "number", value: "5555555555554444" },
    { role: "verificationNumber", value: "456" },
    { role: "expiryMonth", value: "11" },
    { role: "expiryYear", value: "2034" },
  ];

  it("refills every field after a site's mask reformatted the first fill", () => {
    page(`<form><input id="num" autocomplete="cc-number"><input id="exp" autocomplete="cc-exp" placeholder="MMYY"><input id="cvv" autocomplete="cc-csc"><input id="n" autocomplete="cc-name"></form>`);
    $("#num").addEventListener("input", (e) => {
      const t = e.target as HTMLInputElement;
      t.value = (t.value.replace(/\D/g, "").match(/.{1,4}/g) ?? []).join(" ");
    });
    $("#exp").addEventListener("input", (e) => {
      const t = e.target as HTMLInputElement;
      const d = t.value.replace(/\D/g, "");
      t.value = d.length > 2 ? `${d.slice(0, 2)}/${d.slice(2)}` : d;
    });
    const g = cardGroupFor($("#num"), env)!;
    fillCard(g, VALUES, env);
    expect($("#num").value).toBe("4111 1111 1111 1111");
    expect($("#exp").value).toBe("04/33");
    fillCard(g, V2, env);
    expect($("#num").value).toBe("5555 5555 5555 4444");
    expect($("#exp").value).toBe("11/34");
    expect($("#cvv").value).toBe("456");
    expect($("#n").value).toBe("Ana Lima");
  });

  it("derives the combined expiry format from pattern", () => {
    const f = (p: string) => expiryFor(input(`pattern="${p}"`), "4", "2033");
    expect(f("\\d{2}/\\d{2}")).toBe("04/33");
    expect(f("\\d{2}/\\d{4}")).toBe("04/2033");
    expect(f("[0-9]{2}/[0-9]{2}")).toBe("04/33");
    expect(f("\\d{2}\\s?/\\s?\\d{2}")).toBe("04/33");
    expect(f("\\d{2}-\\d{2}")).toBe("04-33");
    expect(f("\\d{4}")).toBe("0433");
    expect(f("\\d{6}")).toBe("042033");
  });

  it("falls back to the maxlength rule when the shown format does not fit", () => {
    expect(expiryFor(input(`placeholder="MM/AA" maxlength="4"`), "4", "2033")).toBe("0433");
    expect(expiryFor(input(`placeholder="MM/YYYY" maxlength="5"`), "4", "2033")).toBe("04/33");
    expect(expiryFor(input(`pattern="\\d{2}/\\d{4}" maxlength="5"`), "4", "2033")).toBe("04/33");
    expect(expiryFor(input(`placeholder="MMAA"`), "11", "2033")).toBe("1133");
  });

  it("treats a site's pre-selected option as empty, but a real first option as a value", () => {
    page(`<form><input id="num" autocomplete="cc-number"><select id="y" autocomplete="cc-exp-year"><option value="">Ano</option><option>2030</option><option selected>2031</option><option>2033</option></select>
      <select id="m" autocomplete="cc-exp-month"><option>1</option><option>4</option></select></form>`);
    fillCard(cardGroupFor($("#num"), env)!, VALUES, env);
    expect($<HTMLSelectElement>("#y").value).toBe("2033");
    expect($<HTMLSelectElement>("#m").value).toBe("1");
  });

  it("reads January and the first year at index 0, and a month by its name", () => {
    page(`<form><input id="num" autocomplete="cc-number"><select id="m" autocomplete="cc-exp-month"><option value="3">April</option><option value="0">January</option></select>
      <select id="y" autocomplete="cc-exp-year"><option>2033</option><option>2034</option></select></form>`);
    $("#num").value = "4111111111111111";
    markUserEdit($("#num"));
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)?.expiry).toBe("2033-04");
    $<HTMLSelectElement>("#m").selectedIndex = 1;
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)?.expiry).toBe("2033-01");
  });

  it("joins given and family name fields when read", () => {
    page(`<form><input id="num" autocomplete="cc-number"><input id="exp" autocomplete="cc-exp"><input id="g" autocomplete="cc-given-name"><input id="f" autocomplete="cc-family-name"></form>`);
    for (const [s, v] of [["#num", "4111111111111111"], ["#exp", "04/33"]] as const) {
      $(s).value = v;
      markUserEdit($(s));
    }
    $("#g").value = "Samuel";
    $("#f").value = "Rocha";
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)?.cardholderName).toBe("Samuel Rocha");
  });

  it("groups an Amex number 4-6-5 and cuts a text year to 2 digits", () => {
    page(`<form><input id="num" placeholder="0000 000000 00000" autocomplete="cc-number"><input id="y" autocomplete="cc-exp-year" maxlength="2"></form>`);
    fillCard(cardGroupFor($("#num"), env)!, [{ role: "number", value: "371449635398431" }, { role: "expiryYear", value: "2033" }], env);
    expect($("#num").value).toBe("3714 496353 98431");
    expect($("#y").value).toBe("33");
  });

  it("knows brand codes and 3-letter Portuguese months", () => {
    page(`<select id="b"><option value="">--</option><option value="AMEX">Amex</option><option value="HC">Hiper</option></select>
      <select id="a"><option value="">--</option><option value="AX">x</option></select>
      <select id="m"><option value="">--</option><option>Mar</option><option>Abr</option></select>`);
    expect(matchCardOption($("#b"), "brand", "amex")).toBe(1);
    expect(matchCardOption($("#b"), "brand", "hipercard")).toBe(2);
    expect(matchCardOption($("#a"), "brand", "amex")).toBe(1);
    expect(matchCardOption($("#m"), "expiryMonth", "4")).toBe(2);
  });
});

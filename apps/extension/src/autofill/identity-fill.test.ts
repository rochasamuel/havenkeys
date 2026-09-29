// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { Env } from "./group";
import { identityGroupFor } from "./identity";
import { fillIdentity, matchOption, rolesToFill } from "./identity-fill";

let hiddenIds = new Set<string>();
const env: Env = { isVisible: (el) => !el.hidden && !hiddenIds.has(el.id), path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = <T extends HTMLElement = HTMLInputElement>(sel: string) => document.querySelector(sel) as T;

beforeEach(() => {
  page("");
  hiddenIds = new Set();
});

const FORM = `<form>
  <input id="n" name="nome_completo" aria-label="Nome completo">
  <input id="c" name="cep" aria-label="CEP">
  <select id="uf" name="uf" aria-label="UF"><option value="">--</option><option value="SP">São Paulo</option><option value="DF">Distrito Federal</option></select>
  <select id="pais" name="pais" aria-label="País"><option value="US">United States</option><option value="BR">Brasil</option></select>
  <input id="t" name="celular" type="tel" maxlength="15">
  <input id="d" type="date" aria-label="Data de nascimento">
</form>`;

const group = () => identityGroupFor($("#n"), env)!;

describe("fillIdentity", () => {
  it("fills empty fields and fires input and change", () => {
    page(FORM);
    const events: string[] = [];
    $("#n").addEventListener("input", () => events.push("input"));
    $("#n").addEventListener("change", () => events.push("change"));
    const n = fillIdentity(group(), [{ role: "fullName", value: "Samuel Rocha" }, { role: "postalCode", value: "71266-105" }], env);
    expect(n).toBe(2);
    expect($("#n").value).toBe("Samuel Rocha");
    expect($("#c").value).toBe("71266-105");
    expect(events).toEqual(["input", "change"]);
    expect(document.body.innerHTML).not.toContain("Samuel");
  });

  it("never overwrites what the user typed, but updates its own values", () => {
    page(FORM);
    $("#n").value = "Typed by me";
    expect(fillIdentity(group(), [{ role: "fullName", value: "Samuel" }, { role: "postalCode", value: "1" }], env)).toBe(1);
    expect($("#n").value).toBe("Typed by me");
    expect(fillIdentity(group(), [{ role: "postalCode", value: "2" }], env)).toBe(1);
    expect($("#c").value).toBe("2");
  });

  it("skips a field hidden after the menu opened", () => {
    page(FORM);
    const g = group();
    hiddenIds.add("c");
    fillIdentity(g, [{ role: "postalCode", value: "71266-105" }], env);
    expect($("#c").value).toBe("");
  });

  it("puts a birth date into a date input as ISO", () => {
    page(FORM);
    fillIdentity(group(), [{ role: "birthDate", value: "2000-04-20" }], env);
    expect($("#d").value).toBe("2000-04-20");
  });

  it("retries a phone without the country code when maxlength is short", () => {
    page(FORM);
    fillIdentity(group(), [{ role: "phone", value: "+55 61 99999-0000" }], env);
    expect($("#t").value).toBe("61 99999-0000");
    page(FORM.replace('maxlength="15"', 'maxlength="5"'));
    expect(fillIdentity(group(), [{ role: "phone", value: "+55 61 99999-0000" }], env)).toBe(0);
    expect($("#t").value).toBe("");
  });
});

describe("select matching", () => {
  it("matches state names, UF codes and countries, ignoring case and accents", () => {
    page(FORM);
    const uf = $<HTMLSelectElement>("#uf");
    const pais = $<HTMLSelectElement>("#pais");
    expect(matchOption(uf, "state", "DF")).toBe(2);
    expect(matchOption(uf, "state", "distrito federal")).toBe(2);
    expect(matchOption(uf, "state", "Sao Paulo")).toBe(1);
    expect(matchOption(uf, "state", "Texas")).toBe(-1);
    expect(matchOption(pais, "country", "Brasil")).toBe(1);
    expect(matchOption(pais, "country", "Brazil")).toBe(1);
    expect(matchOption(pais, "country", "BR")).toBe(1);
  });

  it("selects the match and leaves an unmatched select alone", () => {
    page(FORM);
    fillIdentity(group(), [{ role: "state", value: "Distrito Federal" }, { role: "country", value: "Narnia" }], env);
    expect($<HTMLSelectElement>("#uf").value).toBe("DF");
    expect($<HTMLSelectElement>("#pais").value).toBe("US");
  });
});

describe("fillIdentity extras", () => {
  it("writes street, number and complement on separate lines into a textarea", () => {
    page(`<form><input id="n" name="nome_completo" aria-label="Nome completo"><textarea id="a" autocomplete="street-address"></textarea></form>`);
    const g = identityGroupFor($("#n"), env)!;
    const ta = $<HTMLTextAreaElement>("#a");
    expect(fillIdentity(g, [{ role: "street", value: "Rua A" }, { role: "number", value: "10" }, { role: "complement", value: "" }], env)).toBe(1);
    expect(ta.value).toBe("Rua A\n10");
    ta.value = "";
    fillIdentity(g, [{ role: "addressLine1", value: "Rua A, 10" }], env);
    expect(ta.value).toBe("Rua A, 10");
  });

  it("goes through the prototype setter so React's tracker sees the change", () => {
    page(FORM);
    const el = $("#n");
    let tracked = "";
    Object.defineProperty(el, "value", { configurable: true, get: () => tracked, set: (v: string) => { tracked = v; } });
    let seen = "";
    el.addEventListener("input", () => {
      seen = (Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.get?.call(el) as string) ?? "";
    });
    fillIdentity(group(), [{ role: "fullName", value: "Samuel" }], env);
    expect(seen).toBe("Samuel");
    expect(tracked).toBe("");
  });
});

describe("rolesToFill", () => {
  it("drops fields the user typed into and keeps ones we filled", () => {
    page(FORM);
    const g = group();
    expect(rolesToFill(g, env)).toEqual(expect.arrayContaining(["fullName", "postalCode"]));
    fillIdentity(g, [{ role: "fullName", value: "Samuel" }], env);
    $("#c").value = "typed";
    const roles = rolesToFill(g, env);
    expect(roles).toContain("fullName");
    expect(roles).not.toContain("postalCode");
  });

  it("drops hidden fields", () => {
    page(FORM);
    const g = group();
    hiddenIds.add("c");
    expect(rolesToFill(g, env)).not.toContain("postalCode");
  });
});

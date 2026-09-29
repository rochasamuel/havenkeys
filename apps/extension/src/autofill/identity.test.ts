// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { Env } from "./group";
import { findIdentityGroup, identityGroupFor, identityRoleOf, rolesOf } from "./identity";

function visible(el: HTMLElement): boolean {
  for (let n: HTMLElement | null = el; n; n = n.parentElement) {
    if (n.hidden) return false;
    const s = getComputedStyle(n);
    if (s.display === "none" || s.visibility === "hidden") return false;
  }
  return true;
}
const env: Env = { isVisible: visible, path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = <T extends Element = HTMLInputElement>(sel: string) => document.querySelector(sel) as T;
const role = (sel: string) => identityRoleOf($(sel))?.role ?? null;

beforeEach(() => page(""));

const BR_CHECKOUT = `<form>
  <label for="n">Nome completo</label><input id="n" name="nome">
  <label for="c">CPF</label><input id="c" name="cpf">
  <label for="t">Celular</label><input id="t" name="celular" type="tel">
  <label for="z">CEP</label><input id="z" name="cep">
  <label for="r">Logradouro</label><input id="r" name="logradouro">
  <label for="u">Número</label><input id="u" name="numero">
  <label for="m">Complemento</label><input id="m" name="complemento">
  <label for="b">Bairro</label><input id="b" name="bairro">
  <label for="d">Cidade</label><input id="d" name="cidade">
  <label for="s">UF</label><select id="s" name="uf"><option value="">--</option><option value="DF">DF</option></select>
  <button type="submit">Finalizar compra</button></form>`;

describe("identityRoleOf", () => {
  it("classifies a Brazilian checkout by its words", () => {
    page(BR_CHECKOUT);
    expect(role("#n")).toBe("fullName");
    expect(role("#c")).toBe("cpf");
    expect(role("#t")).toBe("phone");
    expect(role("#z")).toBe("postalCode");
    expect(role("#r")).toBe("street");
    expect(role("#u")).toBe("number");
    expect(role("#m")).toBe("complement");
    expect(role("#b")).toBe("neighborhood");
    expect(role("#d")).toBe("city");
    expect(role("#s")).toBe("state");
  });

  it("trusts autocomplete first, with section and shipping prefixes", () => {
    page(`<form>
      <input id="a" autocomplete="shipping given-name" name="x1">
      <input id="b" autocomplete="section-x billing family-name" name="x2">
      <input id="c" autocomplete="shipping address-line1" name="x3">
      <input id="d" autocomplete="postal-code" name="x4">
      <input id="e" autocomplete="bday-day" name="x5">
      <textarea id="f" autocomplete="street-address"></textarea>
      <input id="g" autocomplete="tel-national" name="x6">
    </form>`);
    expect(role("#a")).toBe("firstName");
    expect(role("#b")).toBe("lastName");
    expect(role("#c")).toBe("addressLine1");
    expect(role("#d")).toBe("postalCode");
    expect(role("#e")).toBe("birthDay");
    expect(role("#f")).toBe("addressLine1");
    expect(role("#g")).toBe("phone");
  });

  it("never classifies passwords, codes, cards or search boxes", () => {
    page(`<form>
      <input id="p" type="password" name="nome">
      <input id="o" autocomplete="one-time-code" name="cep">
      <input id="k" autocomplete="cc-name" name="nome">
      <input id="q" type="search" name="cidade">
      <input id="h" type="hidden" name="cpf">
    </form>`);
    for (const id of ["#p", "#o", "#k", "#q", "#h"]) expect(role(id), id).toBeNull();
  });

  it("refuses card-number fields named cc", () => {
    page(`<form><input id="a" name="cc_number"><input id="b" name="cc"><input id="c" name="nome"></form>`);
    expect(role("#a")).toBeNull();
    expect(role("#b")).toBeNull();
    expect(role("#c")).toBe("fullName");
  });

  it("reads a birth date input and English labels", () => {
    page(`<form><label>Date of birth<input id="b" type="date"></label><label>ZIP code<input id="z"></label></form>`);
    expect(role("#b")).toBe("birthDate");
    expect(role("#z")).toBe("postalCode");
  });
});

describe("identity groups", () => {
  it("qualify with two identity fields", () => {
    page(BR_CHECKOUT);
    const g = identityGroupFor($("#z"), env);
    expect(g).not.toBeNull();
    expect(rolesOf(g!)).toEqual(["fullName", "cpf", "phone", "postalCode", "street", "number", "complement", "neighborhood", "city", "state"]);
  });

  it("qualify with one field whose autocomplete names a non-email role", () => {
    page(`<form><input id="z" autocomplete="postal-code"></form>`);
    expect(identityGroupFor($("#z"), env)).not.toBeNull();
  });

  it("do not qualify for a newsletter's lone email box", () => {
    page(`<form><input id="e" type="email" autocomplete="email" name="email"><button>Subscribe</button></form>`);
    expect(identityGroupFor($("#e"), env)).toBeNull();
  });

  it("skip hidden and disabled fields", () => {
    page(`<form><input id="n" name="nome"><input id="c" name="cpf" hidden><input id="z" name="cep" disabled></form>`);
    expect(identityGroupFor($("#n"), env)).toBeNull();
  });

  it("findIdentityGroup picks the first qualifying form, bounded", () => {
    page(`<form><input name="q" type="search"></form>${BR_CHECKOUT}`);
    const g = findIdentityGroup(document, env);
    expect(g && rolesOf(g)[0]).toBe("fullName");
    page(`<form>${"<input name=x>".repeat(5000)}</form>`);
    expect(findIdentityGroup(document, env)).toBeNull();
  });

  it("adds street, number and complement for a street-address textarea", () => {
    page(`<form><input id="n" name="nome"><textarea id="f" autocomplete="street-address"></textarea></form>`);
    const g = identityGroupFor($("#n"), env)!;
    expect(rolesOf(g)).toEqual(["fullName", "addressLine1", "street", "number", "complement"]);
  });
});

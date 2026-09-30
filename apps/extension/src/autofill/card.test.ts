// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { Env } from "./group";
import { groupFor } from "./group";
import { identityRoleOf } from "./identity";
import { cardFieldsForFill, cardGroupFor, cardRolesOf } from "./card";

const env: Env = { isVisible: (el) => !el.hidden, path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = <T extends HTMLElement = HTMLInputElement>(sel: string) => document.querySelector(sel) as T;
const kinds = (sel: string) => cardGroupFor($(sel), env)?.fields.map((f) => [f.el.id, f.kind]) ?? null;
const months = Array.from({ length: 12 }, (_, i) => `<option value="${i + 1}">${String(i + 1).padStart(2, "0")}</option>`).join("");
const years = Array.from({ length: 12 }, (_, i) => `<option>${2026 + i}</option>`).join("");

beforeEach(() => page(""));

describe("card classifier", () => {
  it("reads a Brazilian checkout by its words", () => {
    page(`<form>
      <input id="num" name="numero_cartao" aria-label="Número do cartão" inputmode="numeric" maxlength="19">
      <input id="nome" name="nome_impresso" aria-label="Nome impresso no cartão">
      <input id="val" name="validade" placeholder="MM/AA" maxlength="5">
      <input id="cvv" name="cvv" aria-label="CVV" maxlength="4">
      <button>Pagar</button></form>`);
    expect(kinds("#num")).toEqual([["num", "number"], ["nome", "cardholderName"], ["val", "expiry"], ["cvv", "verificationNumber"]]);
    expect(cardRolesOf(cardGroupFor($("#num"), env)!.fields)).toEqual(["number", "cardholderName", "expiryMonth", "expiryYear", "verificationNumber"]);
  });

  it("reads a US checkout by autocomplete, month and year selects included", () => {
    page(`<form>
      <input id="cn" autocomplete="cc-name"><input id="cc" autocomplete="billing cc-number">
      <select id="m" autocomplete="cc-exp-month">${months}</select><select id="y" autocomplete="cc-exp-year">${years}</select>
      <input id="csc" type="password" autocomplete="cc-csc" maxlength="4"></form>`);
    expect(kinds("#cc")).toEqual([["cn", "cardholderName"], ["cc", "number"], ["m", "expiryMonth"], ["y", "expiryYear"], ["csc", "verificationNumber"]]);
  });

  it("finds unnamed month and year selects only beside a card number", () => {
    page(`<form><input id="num" aria-label="Card number"><select id="m" name="mm">${months}</select><select id="y" name="yy">${years}</select></form>`);
    expect(kinds("#num")).toEqual([["num", "number"], ["m", "expiryMonth"], ["y", "expiryYear"]]);
    // A birth month on a sign-up form is no card field.
    page(`<form><input id="n" name="nome" aria-label="Nome"><select id="m" name="mes">${months}</select></form>`);
    expect(kinds("#n")).toBeNull();
  });

  it("a processor frame's lone field qualifies when autocomplete names it", () => {
    page(`<input id="n" name="cardnumber" autocomplete="cc-number" inputmode="numeric">`);
    expect(kinds("#n")).toEqual([["n", "number"]]);
    page(`<input id="c" name="cvc" autocomplete="cc-csc">`);
    expect(kinds("#c")).toEqual([["c", "verificationNumber"]]);
    // A lone "month" box with no card context is nothing.
    page(`<input id="m" name="month">`);
    expect(kinds("#m")).toBeNull();
  });

  it("splits a number typed into four boxes, and Amex's 4-6-5", () => {
    page(`<form><label for="c1">Card number</label><input id="c1" maxlength="4"><input id="c2" maxlength="4"><input id="c3" maxlength="4"><input id="c4" maxlength="4"><input id="cvv" name="cvv" maxlength="3"></form>`);
    const g = cardGroupFor($("#c1"), env)!;
    expect(g.fields.filter((f) => f.kind === "number").map((f) => f.shape)).toEqual([
      { kind: "slice", start: 0, length: 4 },
      { kind: "slice", start: 4, length: 4 },
      { kind: "slice", start: 8, length: 4 },
      { kind: "slice", start: 12, length: 4 },
    ]);
    page(`<form><label for="a1">Card number</label><input id="a1" maxlength="4"><input id="a2" maxlength="6"><input id="a3" maxlength="5"></form>`);
    expect(cardGroupFor($("#a1"), env)!.fields.map((f) => f.shape)).toEqual([
      { kind: "slice", start: 0, length: 4 },
      { kind: "slice", start: 4, length: 6 },
      { kind: "slice", start: 10, length: 5 },
    ]);
  });

  it("refuses gift cards, search boxes, hidden and read-only fields", () => {
    page(`<form><input id="g" aria-label="Gift card number"><input id="s" type="search" aria-label="Card number">
      <input id="h" autocomplete="cc-number" hidden><input id="r" autocomplete="cc-number" readonly></form>`);
    for (const id of ["#g", "#s", "#h", "#r"]) expect(kinds(id), id).toBeNull();
  });

  it("the login and identity classifiers never claim a card field", () => {
    page(`<form><input id="num" name="numero_cartao" aria-label="Número do cartão"><input id="nome" aria-label="Nome impresso no cartão"><input id="cvv" name="cvv"></form>`);
    expect(groupFor($("#num"), env).kind).toBe("unknown");
    expect(identityRoleOf($("#nome"))).toBeNull();
    expect(identityRoleOf($("#cvv"))).toBeNull();
  });

  it("for a frame the user did not click, fills every card field of the page", () => {
    page(`<input id="e" autocomplete="cc-exp">`);
    expect(cardFieldsForFill(document, env)?.fields.map((f) => f.kind)).toEqual(["expiry"]);
    page(`<input id="x" name="q">`);
    expect(cardFieldsForFill(document, env)).toBeNull();
  });

  it("a Parcelas select of 1x to 12x is no expiry month, even beside card fields", () => {
    const parcelas = Array.from({ length: 12 }, (_, i) => `<option value="${i + 1}">${i + 1}x</option>`).join("");
    page(`<form><input id="num" aria-label="Número do cartão"><label for="p">Parcelas</label><select id="p">${parcelas}</select><input id="cvv" name="cvv"></form>`);
    expect(kinds("#num")!.map(([id]) => id)).toEqual(["num", "cvv"]);
    // Unlabelled, its options still are not months.
    page(`<form><input id="num" aria-label="Card number"><select id="p" name="x">${parcelas}</select></form>`);
    expect(kinds("#num")).toEqual([["num", "number"]]);
  });

  it("a month select needs exactly 12 months, a year select 10 consecutive years", () => {
    const m13 = months + `<option value="13">13</option><option value="14">14</option>`;
    const placeholder = `<option value="">Mês</option>`;
    page(`<form><input id="num" aria-label="Card number"><select id="a" name="a">${m13}</select><select id="b" name="b">${placeholder}${months}</select>
      <select id="y" name="y">${placeholder}${years}</select></form>`);
    expect(kinds("#num")).toEqual([["num", "number"], ["b", "expiryMonth"], ["y", "expiryYear"]]);
    const sparse = [2026, 2028, 2030, 2032, 2034, 2036, 2038, 2040, 2042, 2044].map((y) => `<option>${y}</option>`).join("");
    page(`<form><input id="num" aria-label="Card number"><select id="s" name="s">${sparse}</select></form>`);
    expect(kinds("#num")).toEqual([["num", "number"]]);
  });

  it("a cardholder-name field that asks for a document, birth date or contact is not one", () => {
    for (const label of ["CPF do titular", "Data de nascimento do titular", "Cardholder email", "Telefone do titular", "Cardholder phone"]) {
      page(`<form><input id="num" aria-label="Card number"><input id="n" aria-label="${label}"></form>`);
      expect(kinds("#num"), label).toEqual([["num", "number"]]);
    }
    page(`<form><input id="num" aria-label="Card number"><input id="n" aria-label="Nome do titular"></form>`);
    expect(kinds("#num")).toEqual([["num", "number"], ["n", "cardholderName"]]);
  });

  it("installment words refuse a field as a card field", () => {
    for (const word of ["Parcelas", "Installments", "Quantidade", "Vezes"]) {
      page(`<form><input id="num" aria-label="Card number"><input id="q" aria-label="${word} validade"></form>`);
      expect(kinds("#num"), word).toEqual([["num", "number"]]);
    }
  });

  it("the identity and login classifiers refuse whatever the card classifier claims", () => {
    page(`<form><input id="t" type="tel" name="cardnumber"><input id="c" type="tel" name="cvv"><input id="e" type="text" autocomplete="cc-exp"></form>`);
    for (const id of ["#t", "#c", "#e"]) expect(identityRoleOf($(id)), id).toBeNull();
    page(`<form><input id="t" type="tel" name="cardnumber"><input id="p" type="password"></form>`);
    expect(groupFor($("#t"), env).kind).toBe("unknown");
    page(`<form><input id="t" type="text" name="cardnumber"><input id="nm" name="username"></form>`);
    expect(groupFor($("#t"), env).kind).toBe("unknown");
    // A plain phone box is still an identity phone.
    page(`<form><input id="ph" type="tel" name="phone"></form>`);
    expect(identityRoleOf($("#ph"))?.role).toBe("phone");
  });

  it("a 2FA security-code field stays a one-time code, not a card field", () => {
    page(`<form><label for="c">Security code</label><input id="c" name="code" autocomplete="one-time-code"><button>Verify</button></form>`);
    expect(groupFor($("#c"), env).kind).toBe("otp");
    expect(cardGroupFor($("#c"), env)).toBeNull();
    page(`<form><label for="c">Código de segurança</label><input id="c" name="code" autocomplete="one-time-code"></form>`);
    expect(groupFor($("#c"), env).kind).toBe("otp");
    page(`<form><label for="c">Security code</label><input id="c" type="password" name="code"></form>`);
    expect(groupFor($("#c"), env).kind).not.toBe("unknown");
  });

  it("ambiguous words claim a field only inside a strong card group", () => {
    page(`<form><label for="u">Titular</label><input id="u" name="username"><input id="p" type="password"><button>Entrar</button></form>`);
    expect(groupFor($("#u"), env).kind).toBe("username");
    page(`<form><label for="i">Customer</label><input id="i" name="cid"></form>`);
    expect(identityRoleOf($("#i"))).toBeNull(); // no role by words, but not refused by card
    page(`<form><input id="n" aria-label="Card number"><label for="s">Security code</label><input id="s"><label for="t">Titular</label><input id="t"></form>`);
    expect(kinds("#n")).toEqual([["n", "number"], ["s", "verificationNumber"], ["t", "cardholderName"]]);
    page(`<form><input id="n" name="cid" aria-label="Customer ID"><input id="e" aria-label="Email" type="email"></form>`);
    expect(cardGroupFor($("#n"), env)).toBeNull();
  });

  it("a birth-date placeholder DD/MM/AA is not an expiry", () => {
    page(`<form><input id="b" name="dt" placeholder="DD/MM/AA"></form>`);
    expect(cardGroupFor($("#b"), env)).toBeNull();
    page(`<form><input id="b" name="nasc" placeholder="Data de nascimento MM/AA"></form>`);
    expect(cardGroupFor($("#b"), env)).toBeNull();
    page(`<form><input id="b" name="dt" placeholder="DD/MM/YY" autocomplete="bday"></form>`);
    expect(identityRoleOf($("#b"))?.role).toBe("birthDate");
  });
});

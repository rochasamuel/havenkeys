// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import { bankRoleOf, classifyBank, type BankGroupFacts } from "./bank";
import { classifyGroup, type Env } from "./group";

const env: Env = { isVisible: (el) => !el.hidden, path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = (sel: string) => document.querySelector(sel) as HTMLInputElement;
const facts = (f: Partial<BankGroupFacts> = {}): BankGroupFacts => ({ groupHasBranch: false, groupHasAccount: false, prevRole: null, ...f });
const box = (label: string, attrs = "") => {
  page(`<label for="x">${label}</label><input id="x" ${attrs}>`);
  return $("#x");
};

beforeEach(() => page(""));

describe("bank classifier", () => {
  it("conta_without_branch_is_not_bank", () => {
    expect(bankRoleOf(box("Conta", 'inputmode="numeric" maxlength="7"'), facts())).toBeNull();
    expect(bankRoleOf(box("Conta", 'inputmode="numeric" maxlength="7"'), facts({ groupHasBranch: true }))).toBe("bankAccount");
  });

  it("entrar_na_sua_conta_is_not_bank", () => {
    expect(bankRoleOf(box("Entrar na sua conta"), facts({ groupHasBranch: true }))).toBeNull();
    expect(bankRoleOf(box("Criar conta"), facts({ groupHasBranch: true }))).toBeNull();
  });

  it("agencia_de_viagem_is_not_bank", () => {
    for (const label of ["Agência de viagem", "Agência de turismo", "Travel agency"]) {
      expect(bankRoleOf(box(label, 'maxlength="6"'), facts({ groupHasAccount: true })), label).toBeNull();
    }
  });

  it("text_ag_box_needs_account_neighbour", () => {
    expect(bankRoleOf(box("Ag", 'type="text"'), facts())).toBeNull();
    expect(bankRoleOf(box("Ag", 'type="text"'), facts({ groupHasAccount: true }))).toBe("bankBranch");
  });

  it("a numeric branch box stands alone (Sicoob's cooperativa, BB's 6-long box)", () => {
    expect(bankRoleOf(box("Digite sua cooperativa", 'type="number" maxlength="4"'), facts())).toBe("bankBranch");
    expect(bankRoleOf(box("Agência", 'name="dependenciaOrigem" maxlength="6"'), facts())).toBe("bankBranch");
  });

  it("short words match whole words only", () => {
    expect(bankRoleOf(box("Page", 'maxlength="4"'), facts({ groupHasAccount: true }))).toBeNull();
    expect(bankRoleOf(box("Advogado"), facts({ prevRole: "bankAccount" }))).toBeNull();
    expect(bankRoleOf(box("Digital"), facts({ prevRole: "bankAccount" }))).toBeNull();
  });

  it("one_char_box_after_account_is_account_digit", () => {
    expect(bankRoleOf(box("", 'maxlength="1"'), facts({ groupHasBranch: true, prevRole: "bankAccount" }))).toBe("bankAccountDigit");
    expect(bankRoleOf(box("DV"), facts({ groupHasAccount: true, prevRole: "bankBranch" }))).toBe("bankBranchDigit");
    expect(bankRoleOf(box("", 'maxlength="1"'), facts({ groupHasBranch: true }))).toBeNull();
  });

  it("password_and_otp_boxes_are_never_bank", () => {
    const g = facts({ groupHasBranch: true, groupHasAccount: true, prevRole: "bankAccount" });
    expect(bankRoleOf(box("Agência", 'type="password" maxlength="4"'), g)).toBeNull();
    expect(bankRoleOf(box("Dígito", 'type="password" maxlength="1"'), g)).toBeNull();
    expect(bankRoleOf(box("Agência", 'autocomplete="one-time-code" maxlength="4"'), g)).toBeNull();
    expect(bankRoleOf(box("Conta", 'type="hidden"'), g)).toBeNull();
    expect(bankRoleOf(box("Conta", "disabled"), g)).toBeNull();
    expect(bankRoleOf(box("Conta", "readonly"), g)).toBeNull();
    expect(bankRoleOf(box("Conta", 'autocomplete="username"'), g)).toBeNull();
    expect(bankRoleOf(box("Número da conta do cartão", 'autocomplete="cc-number"'), g)).toBeNull();
  });

  it("classifyBank reads Itaú's form; the login classifier gives neither box a username", () => {
    page(`<form>
      <label for="ag">Agência</label><input id="ag" type="text" inputmode="numeric" maxlength="4" autocomplete="off">
      <label for="ct">Conta</label><input id="ct" type="text" inputmode="numeric" maxlength="7" autocomplete="off">
      <button type="submit">Acessar</button>
    </form>`);
    const form = document.querySelector("form") as HTMLFormElement;
    expect(classifyBank(form, env).map((b) => [b.el.id, b.role])).toEqual([["ag", "bankBranch"], ["ct", "bankAccount"]]);
    expect(classifyGroup(form, env).fields.filter((f) => f.kind === "username")).toEqual([]);
  });

  it("classifyBank reads Bradesco's digit box and keeps facts per form", () => {
    page(`<form id="a">
      <label for="AGN">Agência:</label><input id="AGN" name="AGN" type="number" maxlength="4">
      <label for="CTA">Conta:</label><input id="CTA" name="CTA" type="number" maxlength="7">
      <label for="DIGCTA">Dígito:</label><input id="DIGCTA" name="DIGCTA" type="number" maxlength="1">
    </form>
    <form id="b"><label for="c2">Conta</label><input id="c2" maxlength="7"></form>`);
    expect(classifyBank(document, env).map((b) => [b.el.id, b.role])).toEqual([
      ["AGN", "bankBranch"],
      ["CTA", "bankAccount"],
      ["DIGCTA", "bankAccountDigit"],
    ]);
  });

  it("a sign-in form saying conta keeps its username", () => {
    page(`<form><h2>Entrar na sua conta</h2>
      <label for="u">Usuário ou número da conta</label><input id="u" type="text">
      <label for="p">Senha</label><input id="p" type="password">
    </form>`);
    const form = document.querySelector("form") as HTMLFormElement;
    expect(classifyBank(form, env)).toEqual([]);
    expect(classifyGroup(form, env).fields.find((f) => f.el.id === "u")?.kind).toBe("username");
  });
});

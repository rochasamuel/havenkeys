// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BankValues } from "@havenkeys/protocol";
import { fillBank, shapeBank } from "./bank-fill";
import type { Env } from "./group";

const env: Env = { isVisible: (el) => !el.hidden, path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = (sel: string) => document.querySelector(sel) as HTMLInputElement;
const values: BankValues = { branch: "1234", branchDigit: null, account: "12345", accountDigit: "6" };

describe("shapeBank", () => {
  const text = (maxLength: number | null, hasDigitBox = false) => ({ type: "text", maxLength, hasDigitBox });
  it.each([
    ["12345", "6", text(7), "12345-6"],
    ["12345", "6", text(6), "123456"],
    ["12345", "6", text(5), "12345"],
    ["12345", "6", { type: "number", maxLength: null, hasDigitBox: false }, "123456"],
    ["12345", "X", { type: "number", maxLength: null, hasDigitBox: false }, "12345"],
    ["12345", "6", text(null, true), "12345"],
    ["1234", "X", text(6), "1234-X"],
    ["12345", null, text(7), "12345"],
    ["12345", "6", text(null), "12345-6"],
  ])("%s + %s in %j gives %s", (value, digit, box, expected) => {
    expect(shapeBank(value, digit, box)).toBe(expected);
  });
});

describe("fillBank", () => {
  beforeEach(() => page(""));

  it("fills_bradesco_three_boxes", () => {
    page(`<form>
      <label for="AGN">Agência:</label><input id="AGN" type="number" maxlength="4">
      <label for="CTA">Conta:</label><input id="CTA" type="number" maxlength="7">
      <label for="DIGCTA">Dígito:</label><input id="DIGCTA" type="number" maxlength="1">
    </form>`);
    expect(fillBank(document, $("#AGN"), values, null, env)).toBe(3);
    expect([$("#AGN").value, $("#CTA").value, $("#DIGCTA").value]).toEqual(["1234", "12345", "6"]);
  });

  it("fills_itau_with_mask", () => {
    page(`<form>
      <label for="ag">Agência</label><input id="ag" type="text" inputmode="numeric" maxlength="4">
      <label for="ct">Conta</label><input id="ct" type="text" inputmode="numeric" maxlength="7">
    </form>`);
    $("#ct").addEventListener("input", (e) => {
      const el = e.target as HTMLInputElement;
      const d = el.value.replace(/\D/g, "");
      el.value = d.length > 1 ? `${d.slice(0, -1)}-${d.slice(-1)}` : d;
    });
    expect(fillBank(document, $("#ag"), values, null, env)).toBe(2);
    expect($("#ct").value).toBe("12345-6");
    // The masked value is still ours: a second pick replaces it.
    expect(fillBank(document, $("#ag"), { ...values, account: "99999" }, null, env)).toBe(2);
    expect($("#ct").value).toBe("99999-6");
  });

  it("fills_password_in_same_form_only", () => {
    page(`<form id="a">
      <label for="ag">Agência</label><input id="ag" type="text" maxlength="4">
      <label for="ct">Conta</label><input id="ct" type="text" maxlength="7">
      <label for="p1">Senha</label><input id="p1" type="password">
    </form>
    <form id="b"><label for="p2">Senha</label><input id="p2" type="password"></form>`);
    expect(fillBank(document, $("#ag"), values, "s3nha", env)).toBe(3);
    expect($("#p1").value).toBe("s3nha");
    expect($("#p2").value).toBe("");
  });

  it("skips_hidden_and_prefilled_boxes", () => {
    page(`<form>
      <label for="ag">Agência</label><input id="ag" type="text" maxlength="4" value="9999">
      <label for="ct">Conta</label><input id="ct" type="text" maxlength="7">
      <label for="p">Senha</label><input id="p" type="password" hidden>
    </form>`);
    expect(fillBank(document, $("#ag"), values, "s3nha", env)).toBe(1);
    expect([$("#ag").value, $("#ct").value, $("#p").value]).toEqual(["9999", "12345-6", ""]);
    $("#ct").hidden = true;
    $("#ct").value = "";
    expect(fillBank(document, $("#ag"), values, null, env)).toBe(0);
    expect($("#ct").value).toBe("");
  });

  it("never_overwrites_a_value_the_user_typed_after_ours", () => {
    page(`<form>
      <label for="ag">Agência</label><input id="ag" type="text" maxlength="4">
      <label for="ct">Conta</label><input id="ct" type="text" maxlength="7">
    </form>`);
    fillBank(document, $("#ag"), values, null, env);
    $("#ct").value = "777";
    expect(fillBank(document, $("#ag"), values, null, env)).toBe(1); // only the agência, still ours
    expect($("#ct").value).toBe("777");
  });

  it("fills_a_conta_only_item_into_its_conta_box_and_password", () => {
    page(`<form>
      <label for="ag">Agência</label><input id="ag" type="text" maxlength="4">
      <label for="ct">Conta</label><input id="ct" type="text" maxlength="7">
      <label for="p">Senha</label><input id="p" type="password">
    </form>`);
    const contaOnly: BankValues = { branch: null, branchDigit: null, account: "12345", accountDigit: "6" };
    expect(fillBank(document, $("#ag"), contaOnly, "s3nha", env)).toBe(2);
    expect([$("#ag").value, $("#ct").value, $("#p").value]).toEqual(["", "12345-6", "s3nha"]);
  });

  it("fills_a_branch_only_item_into_its_branch_boxes", () => {
    page(`<form>
      <label for="ag">Agência</label><input id="ag" type="text" maxlength="4">
      <label for="dag">Dígito da agência</label><input id="dag" type="text" maxlength="1">
      <label for="ct">Conta</label><input id="ct" type="text" maxlength="7">
    </form>`);
    const branchOnly: BankValues = { branch: "1234", branchDigit: "5", account: null, accountDigit: null };
    expect(fillBank(document, $("#ct"), branchOnly, null, env)).toBe(2);
    expect([$("#ag").value, $("#dag").value, $("#ct").value]).toEqual(["1234", "5", ""]);
  });

  it("never_submits", () => {
    page(`<form><label for="ag">Agência</label><input id="ag" type="text" maxlength="4">
      <label for="ct">Conta</label><input id="ct" type="text" maxlength="7"><button type="submit">Entrar</button></form>`);
    const form = document.querySelector("form") as HTMLFormElement;
    const submit = vi.fn((e: Event) => e.preventDefault());
    const click = vi.fn();
    form.addEventListener("submit", submit);
    form.querySelector("button")?.addEventListener("click", click);
    expect(fillBank(document, $("#ag"), values, null, env)).toBe(2);
    expect(submit).not.toHaveBeenCalled();
    expect(click).not.toHaveBeenCalled();
  });
});

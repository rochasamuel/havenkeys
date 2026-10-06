// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { Env } from "./group";
import { identityGroupFor } from "./identity";
import { fillIdentity } from "./identity-fill";
import { isIdentityVisible, type Box, type Layout } from "./visibility";

// jsdom has no layout: a stub gives each element a box and computed style.
type Style = ReturnType<Layout["style"]>;
const DEFAULT_STYLE: Style = { opacity: "1", overflowX: "visible", overflowY: "visible", clip: "auto", clipPath: "none", position: "static" };
let boxes: Map<Element, Box>;
let styles: Map<Element, Partial<Style>>;
let page: ReturnType<Layout["page"]>;
const FIELD_BOX: Box = { left: 100, top: 100, width: 200, height: 30 };
const layout: Layout = {
  rect: (el) => boxes.get(el) ?? { left: 0, top: 0, width: 1000, height: 1000 },
  style: (el) => ({ ...DEFAULT_STYLE, ...(styles.get(el) ?? {}) }),
  page: () => page,
};

const html = (s: string) => (document.body.innerHTML = s);
const $ = (sel: string) => document.querySelector(sel) as HTMLInputElement;
const visible = (sel: string) => isIdentityVisible($(sel), layout);

beforeEach(() => {
  boxes = new Map();
  styles = new Map();
  page = { scrollX: 0, scrollY: 0, width: 1280, height: 2000 };
  html(`<form id="f"><div id="w"><input id="a" name="nome" aria-label="Nome completo"><input id="b" name="celular" aria-label="Celular"></div></form>`);
  boxes.set($("#a"), FIELD_BOX);
  boxes.set($("#b"), { ...FIELD_BOX, top: 140 });
});

describe("isIdentityVisible", () => {
  it("accepts an ordinary field", () => {
    expect(visible("#a")).toBe(true);
  });

  it("accepts a field scrolled out of the viewport but inside the page", () => {
    page = { ...page, scrollY: 1500 };
    boxes.set($("#a"), { ...FIELD_BOX, top: -1400 });
    expect(visible("#a")).toBe(true);
  });

  it("refuses a field moved off the page (left: -9999px)", () => {
    boxes.set($("#a"), { ...FIELD_BOX, left: -9999 });
    expect(visible("#a")).toBe(false);
    boxes.set($("#a"), { ...FIELD_BOX, top: -9999 });
    expect(visible("#a")).toBe(false);
  });

  it("refuses a field past the page's right or bottom edge", () => {
    boxes.set($("#a"), { ...FIELD_BOX, left: 5000 });
    expect(visible("#a")).toBe(false);
    boxes.set($("#a"), { ...FIELD_BOX, top: 9000 });
    expect(visible("#a")).toBe(false);
  });

  it("refuses a nearly transparent field (opacity: 0.01)", () => {
    styles.set($("#a"), { opacity: "0.01" });
    expect(visible("#a")).toBe(false);
  });

  it("refuses a field whose ancestors multiply to under 0.1 opacity", () => {
    styles.set($("#a"), { opacity: "0.3" });
    styles.set($("#w"), { opacity: "0.3" });
    expect(visible("#a")).toBe(false);
    styles.set($("#w"), { opacity: "0.9" });
    expect(visible("#a")).toBe(true);
  });

  it("refuses clip-path: inset(50%) on the field or an ancestor", () => {
    styles.set($("#a"), { clipPath: "inset(50%)" });
    expect(visible("#a")).toBe(false);
    styles.delete($("#a"));
    styles.set($("#w"), { clipPath: "inset(0px 0px 100% 0px)" });
    expect(visible("#a")).toBe(false);
    styles.set($("#w"), { clipPath: "inset(2px round 4px)" });
    expect(visible("#a")).toBe(true);
  });

  it("refuses zero-size circle and polygon clip-paths", () => {
    styles.set($("#a"), { clipPath: "circle(0px at 50% 50%)" });
    expect(visible("#a")).toBe(false);
    styles.set($("#a"), { clipPath: "polygon(0px 0px, 0px 0px, 0px 0px)" });
    expect(visible("#a")).toBe(false);
    styles.set($("#a"), { clipPath: "polygon(0% 0%, 100% 0%, 100% 100%)" });
    expect(visible("#a")).toBe(true);
  });

  it("refuses the screen-reader clip: rect(0 0 0 0) on an absolute field", () => {
    styles.set($("#a"), { clip: "rect(0px, 0px, 0px, 0px)", position: "absolute" });
    expect(visible("#a")).toBe(false);
    styles.set($("#a"), { clip: "rect(1px, 1px, 1px, 1px)", position: "absolute" });
    expect(visible("#a")).toBe(false);
    // clip does nothing on a static element.
    styles.set($("#a"), { clip: "rect(0px, 0px, 0px, 0px)", position: "static" });
    expect(visible("#a")).toBe(true);
  });

  it("refuses a field inside a zero-height overflow: hidden parent", () => {
    styles.set($("#w"), { overflowX: "hidden", overflowY: "hidden" });
    boxes.set($("#w"), { left: 100, top: 100, width: 200, height: 0 });
    expect(visible("#a")).toBe(false);
  });

  it("refuses a field its overflow: hidden parent cuts off", () => {
    styles.set($("#w"), { overflowX: "visible", overflowY: "hidden" });
    boxes.set($("#w"), { left: 100, top: 0, width: 200, height: 50 });
    expect(visible("#a")).toBe(false); // the field starts at y=100
    boxes.set($("#w"), { left: 100, top: 90, width: 200, height: 200 });
    expect(visible("#a")).toBe(true);
  });

  it("stops after a bounded number of ancestors", () => {
    let inner = `<input id="deep" name="cep">`;
    for (let i = 0; i < 40; i++) inner = `<div>${inner}</div>`;
    html(`<div id="top">${inner}</div>`);
    boxes.set($("#deep"), FIELD_BOX);
    styles.set($("#top"), { opacity: "0" });
    let reads = 0;
    const counting: Layout = { ...layout, style: (el) => (reads++, layout.style(el)) };
    expect(isIdentityVisible($("#deep"), counting)).toBe(true);
    expect(reads).toBeLessThanOrEqual(16);
  });
});

describe("identity groups use the strict check", () => {
  const env: Env = { isVisible: () => true, strictVisible: (el) => isIdentityVisible(el, layout), path: "/" };

  it("does not classify, and does not fill, an off-page field", () => {
    boxes.set($("#b"), { ...FIELD_BOX, left: -9999 });
    const g = identityGroupFor($("#a"), { ...env, isVisible: () => true });
    // Only the visible name field is left: one field without autocomplete does not qualify.
    expect(g).toBeNull();
  });

  it("does not write into a field hidden after the menu opened", () => {
    const g = identityGroupFor($("#a"), env)!;
    expect(g.fields).toHaveLength(2);
    styles.set($("#b"), { opacity: "0.01" });
    expect(fillIdentity(g, [{ role: "fullName", value: "Samuel" }, { role: "phone", value: "+55 61 99999-0000" }], env)).toBe(1);
    expect($("#b").value).toBe("");
  });
});

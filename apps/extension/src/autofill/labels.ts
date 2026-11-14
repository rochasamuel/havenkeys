// The visible label of a form field, for the classifiers. Page-controlled
// text: only ever compared against fixed keyword lists, never inserted,
// evaluated or sent anywhere.

import { MAX_HINT_CHARS } from "./text";

/** Ancestors searched for a floating label; elements visited per ancestor. */
const MAX_LEVELS = 3;
const MAX_VISITED = 60;

/** A control a label could belong to. */
function isControl(el: Element): boolean {
  if (el instanceof HTMLInputElement) return el.type !== "hidden";
  return el instanceof HTMLSelectElement || el instanceof HTMLTextAreaElement;
}

/**
 * A `<label>` not linked to anything, inside a small wrapper whose only
 * control is `el` (React's "floating label": `div > div > input` then
 * `label`). A wrapper with a second control has no label to lend: it may be
 * the other field's. Bounded: three ancestors, sixty elements each.
 */
function floatingLabel(el: Element): string {
  let node = el.parentElement;
  for (let level = 0; node && level < MAX_LEVELS; level++, node = node.parentElement) {
    let controls = 0;
    let label: HTMLLabelElement | null = null;
    let labels = 0;
    let visited = 0;
    const walker = document.createTreeWalker(node, NodeFilter.SHOW_ELEMENT);
    for (let n = walker.nextNode() as Element | null; n; n = walker.nextNode() as Element | null) {
      if (++visited > MAX_VISITED) return "";
      if (isControl(n) && ++controls > 1) return "";
      if (n instanceof HTMLLabelElement && !n.htmlFor && !n.control) {
        label = n;
        labels++;
      }
    }
    if (labels === 1 && label) return (label.textContent ?? "").slice(0, MAX_HINT_CHARS);
    if (labels > 1) return "";
  }
  return "";
}

// ------------------------------------------------------- neighbouring text
//
// Plain text placed next to a field with no `<label>`: old table layouts
// (`<td>Username:</td><td><input></td>`) and `<span>Password</span><input>`.
// Text between two controls belongs to the one after it (labels come before
// fields), so text after a field counts only when no control follows it.

/** Levels scanned: the field's own siblings, then those of up to two wrappers holding no other control. */
const MAX_NEIGHBOUR_LEVELS = 3;
/** Sibling nodes visited per level and side (whitespace and comments count). */
const MAX_SIBLINGS = 8;
/** Nodes read inside one neighbouring element. */
const MAX_NEIGHBOUR_NODES = 12;
/** Neighbouring text longer than this is prose, not a label, and is ignored. */
export const MAX_NEIGHBOUR_CHARS = 60;

/** Elements that are never a field's label: reaching one ends the search on that side. */
const NOT_LABEL = new Set([
  "a", "button", "h1", "h2", "h3", "h4", "h5", "h6", "legend", "select", "textarea", "option", "optgroup",
  "script", "style", "template", "noscript", "iframe", "object", "embed", "img", "svg", "canvas", "video",
  "audio", "form", "fieldset", "table", "thead", "tbody", "tr", "ul", "ol", "dialog", "nav", "header",
  "footer", "output", "progress", "meter",
]);

/** Nothing here: read past it. */
const SKIP = 0;
/** Not a label, or ownership unknown: give up on this side. */
const STOP = 1;
type Read = typeof SKIP | typeof STOP | string;

/** A control, a label of another control, or an element that is never a label. */
function endsSearch(n: Element, field: Element): boolean {
  if (isControl(n) || NOT_LABEL.has(n.localName)) return true;
  return n instanceof HTMLLabelElement && (n.htmlFor !== "" || n.control !== null) && n.control !== field;
}

/** Decorative or hidden. */
function skipped(n: Element): boolean {
  return (
    n.localName === "br" ||
    n.localName === "wbr" ||
    (n instanceof HTMLInputElement && n.type === "hidden") ||
    n.hasAttribute("hidden") ||
    n.getAttribute("aria-hidden") === "true"
  );
}

/** Short text inside a neighbouring element; STOP for a control, a non-label, too long or too big. */
function readElement(n: Element, field: Element): Read {
  let text = "";
  let visited = 0;
  const walker = document.createTreeWalker(n, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
  for (let c: Node | null = n; c; c = walker.nextNode()) {
    if (++visited > MAX_NEIGHBOUR_NODES) return STOP;
    if (c.nodeType === Node.TEXT_NODE) {
      text += (c as Text).data.slice(0, MAX_NEIGHBOUR_CHARS * 4);
      if (text.trim().length > MAX_NEIGHBOUR_CHARS) return STOP;
    } else if (endsSearch(c as Element, field)) return STOP;
  }
  const t = text.trim();
  return t === "" ? SKIP : t;
}

function readNode(n: Node, field: Element): Read {
  if (n.nodeType === Node.TEXT_NODE) {
    const t = (n as Text).data.slice(0, MAX_NEIGHBOUR_CHARS * 4).trim();
    if (t === "") return SKIP;
    return t.length > MAX_NEIGHBOUR_CHARS ? STOP : t;
  }
  if (!(n instanceof Element)) return SKIP;
  if (endsSearch(n, field)) return STOP;
  if (skipped(n)) return SKIP;
  return readElement(n, field);
}

/**
 * Does `n` hold a control? Spends from `budget` (elements visited, shared by
 * the caller's checks); out of budget counts as yes.
 */
function holdsControl(n: Node, budget: { left: number }): boolean {
  if (!(n instanceof Element)) return false;
  const walker = document.createTreeWalker(n, NodeFilter.SHOW_ELEMENT);
  for (let c: Node | null = n; c; c = walker.nextNode()) {
    if (--budget.left < 0 || isControl(c as Element)) return true;
  }
  return false;
}

/** Text before `node` among its siblings; SKIP when the siblings ran out with nothing. */
function textBefore(node: Node, field: Element): Read {
  let n = node.previousSibling;
  for (let i = 0; n && i < MAX_SIBLINGS; i++, n = n.previousSibling) {
    const r = readNode(n, field);
    if (r !== SKIP) return r;
  }
  return n ? STOP : SKIP;
}

/** Text after `node` that no later control among its siblings can claim. */
function textAfter(node: Node, field: Element): Read {
  let n = node.nextSibling;
  let found: string | null = null;
  const budget = { left: MAX_VISITED };
  for (let i = 0; n && i < MAX_SIBLINGS; i++, n = n.nextSibling) {
    if (found !== null) {
      // Text followed by a control is that control's label.
      if (holdsControl(n, budget)) return STOP;
      continue;
    }
    const r = readNode(n, field);
    if (r === STOP) return STOP;
    if (typeof r === "string") found = r;
  }
  // Out of budget before the last sibling: who owns the text is unknown.
  if (n) return STOP;
  return found ?? SKIP;
}

/**
 * Short plain text right before the field, else right after it, read from
 * its siblings and then from those of up to two wrappers whose only control
 * is the field (a table cell's preceding cell is its wrapper's sibling; the
 * climb stops at the cell, so another row's text is never read).
 * Never text that a heading, link, button or another control sits between,
 * never a `<label>` of another control, and never longer than
 * MAX_NEIGHBOUR_CHARS. Bounded: 3 levels, 8 siblings a side, 12 nodes per
 * sibling, 60 elements per wrapper check and per "no control follows" check.
 */
export function neighbourText(el: Element): string {
  // Wrappers found on the first side are reused on the second.
  const levels: Array<Element | null> = [el];
  for (const read of [textBefore, textAfter]) {
    for (let level = 0; level < MAX_NEIGHBOUR_LEVELS; level++) {
      const node = levels[level];
      if (!node) break;
      const r = read(node, el);
      if (typeof r === "string") return r.slice(0, MAX_HINT_CHARS);
      if (r === STOP) break;
      if (levels.length === level + 1) levels.push(wrapperOf(node));
    }
  }
  return "";
}

/**
 * `node`'s parent, when nothing beside `node` in it holds a control (`node`
 * itself holds only the field). Never above a table cell: a cell's label is
 * in its own row, not the row above. Bounded: MAX_VISITED elements.
 */
function wrapperOf(node: Element): Element | null {
  if (node.localName === "td" || node.localName === "th") return null;
  const parent = node.parentElement;
  if (!parent || parent === document.body || parent === document.documentElement) return null;
  const budget = { left: MAX_VISITED };
  for (let c = parent.firstElementChild; c; c = c.nextElementSibling) {
    if (c !== node && holdsControl(c, budget)) return null;
  }
  return parent;
}

/**
 * Up to two linked `<label>`s, or else a floating label beside the field,
 * or else short neighbouring text (`neighbourText`).
 */
export function labelText(el: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement): string {
  const linked = Array.from(el.labels ?? []).slice(0, 2).map((l) => (l.textContent ?? "").slice(0, MAX_HINT_CHARS));
  return linked.length ? linked.join(" ") : floatingLabel(el) || neighbourText(el);
}

// The visible label of a form field, for the classifiers. Page-controlled
// text: only ever compared against fixed keyword lists.

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

/** Up to two linked `<label>`s, or else a floating label beside the field. */
export function labelText(el: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement): string {
  const linked = Array.from(el.labels ?? []).slice(0, 2).map((l) => (l.textContent ?? "").slice(0, MAX_HINT_CHARS));
  return linked.length ? linked.join(" ") : floatingLabel(el);
}

// Fields the page itself marks as unavailable to the user, beyond the
// `disabled` property: `aria-disabled="true"` on the field, `aria-hidden="true"`
// on the field or any ancestor (aria-hidden is inherited by the subtree, also
// across shadow roots), and a field inside `<fieldset disabled>` (`:disabled`).
//
// A field like that is never classified, counted or filled. Hidden "honeypot"
// inputs are often marked aria-hidden so screen readers skip them; a page
// that hides a field from assistive technology is telling us the user is
// not meant to type there. No imports: every fillability check (usable.ts,
// group.ts, card.ts, identity.ts, bank.ts) uses this one rule.

/** Ancestors examined for `aria-hidden`, the field included. Past this the field counts as not hidden. */
export const MAX_ARIA_ANCESTORS = 128;

function isTrue(el: Element, name: string): boolean {
  const v = el.getAttribute(name);
  return v !== null && v.trim().toLowerCase() === "true";
}

/**
 * One bounded walk up from `el`: an `aria-hidden="true"` on it or an
 * ancestor (crossing shadow roots to the host), or, with `fieldsets`, a
 * `<fieldset disabled>` ancestor in the same tree whose first `<legend>` does
 * not hold the field (HTML's rule for `:disabled`, without the selector
 * engine's cost).
 */
function hiddenOrFieldsetDisabled(el: Element, fieldsets: boolean): boolean {
  let node: Element | null = el;
  let child: Element | null = null;
  let sameTree = true;
  for (let i = 0; node && i < MAX_ARIA_ANCESTORS; i++) {
    if (isTrue(node, "aria-hidden")) return true;
    if (fieldsets && sameTree && child && node instanceof HTMLFieldSetElement && node.disabled) {
      const legend = Array.prototype.find.call(node.children, (c: Element) => c.localName === "legend");
      if (child !== legend) return true;
    }
    child = node;
    const parent: Element | null = node.parentElement;
    if (parent) node = parent;
    else {
      const root = node.getRootNode();
      node = root instanceof ShadowRoot ? root.host : null;
      sameTree = false;
    }
  }
  return false;
}

/** The field or an ancestor (crossing shadow roots) has `aria-hidden="true"`. Bounded. */
export function isAriaHidden(el: Element): boolean {
  return hiddenOrFieldsetDisabled(el, false);
}

/** Disabled (itself or by a `<fieldset disabled>`), `aria-disabled`, or inside an `aria-hidden` subtree. */
export function isUnavailable(el: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement): boolean {
  return el.disabled || isTrue(el, "aria-disabled") || hiddenOrFieldsetDisabled(el, true);
}

// The in-page layout check, run with page.evaluate. Self-contained: it is
// serialized into the page, so it may not close over anything.
//
// Fails on:
// * a visible element that hides overflow (overflow hidden/clip, or
//   text-overflow: ellipsis) whose content does not fit (scrollWidth >
//   clientWidth + 1, or the same for height) because of text drawn outside
//   its box, unless that text or an ancestor carries data-truncate (user
//   data: titles, usernames, URLs);
// * a button whose text spills out of it (visible overflow, but cut off
//   visually by whatever it runs into);
// * a button that grows past the box it sits in;
// * the page scrolling horizontally, or a visible element sticking out past
//   the viewport's right edge.
export function overflowCheck() {
  const failures = [];
  const vw = window.innerWidth;

  function describe(el) {
    const parts = [];
    let n = el;
    while (n && n.nodeType === 1 && parts.length < 4) {
      let s = n.tagName.toLowerCase();
      if (n.id) s += `#${n.id}`;
      const cls = typeof n.className === "string" ? n.className.trim().split(/\s+/).filter(Boolean).slice(0, 3) : [];
      if (cls.length) s += `.${cls.join(".")}`;
      parts.unshift(s);
      if (n.id) break;
      n = n.parentElement;
    }
    return parts.join(" > ");
  }

  function snippet(el) {
    return (el.innerText || el.textContent || "").replace(/\s+/g, " ").trim().slice(0, 80);
  }

  function visible(el, style, rect) {
    if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") return false;
    if (Number(style.opacity) === 0) return false;
    // Visually hidden helpers (.sr-only) and zero-size boxes.
    if (rect.width <= 1 || rect.height <= 1) return false;
    return true;
  }

  const hides = (v) => v === "hidden" || v === "clip";

  /**
   * Text under `el` that is drawn outside its padding box, i.e. cut off.
   * Decoration (an oversized background SVG) makes scrollWidth/Height
   * larger too but is not text, so it does not count. Text inside a nearer
   * scroll container belongs to that container (it scrolls, it is not cut),
   * and user data marked data-truncate may be cut.
   */
  function clippedText(el, axis) {
    const r = el.getBoundingClientRect();
    const left = r.left + el.clientLeft;
    const top = r.top + el.clientTop;
    const right = left + el.clientWidth;
    const bottom = top + el.clientHeight;
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (!node.data.trim()) continue;
      const parent = node.parentElement;
      if (!parent || parent.closest("[data-truncate]")) continue;
      let own = true;
      for (let p = parent; p && p !== el; p = p.parentElement) {
        const ps = getComputedStyle(p);
        if (/(auto|scroll|hidden|clip)/.test(ps.overflowX + ps.overflowY) || ps.position === "fixed" || ps.display === "none") {
          own = false;
          break;
        }
      }
      if (!own) continue;
      const pstyle = getComputedStyle(parent);
      if (pstyle.visibility !== "visible") continue;
      const range = document.createRange();
      range.selectNodeContents(node);
      for (const t of range.getClientRects()) {
        if (t.width === 0 || t.height === 0) continue;
        const outX = t.right > right + 1 || t.left < left - 1;
        const outY = t.bottom > bottom + 1 || t.top < top - 1;
        if ((axis === "x" && outX) || (axis === "y" && outY)) return node.data.trim().slice(0, 80);
      }
    }
    return null;
  }

  for (const el of document.querySelectorAll("*")) {
    if (!(el instanceof HTMLElement)) continue;
    if (el.closest("svg")) continue;
    const style = getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    if (!visible(el, style, rect)) continue;
    // Skip content inside something already hidden (display:none ancestors
    // give zero rects above; this catches visibility on ancestors).
    if (el.checkVisibility && !el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) continue;

    const truncates = !!el.closest("[data-truncate]");
    const clipX = hides(style.overflowX) || style.textOverflow === "ellipsis";
    const clipY = hides(style.overflowY);
    // Replaced/form elements scroll their own text; inputs show user input.
    const formish = /^(INPUT|TEXTAREA|SELECT|IMG|CANVAS|VIDEO|IFRAME)$/.test(el.tagName);
    if (!truncates && !formish) {
      const cutX = clipX && el.scrollWidth > el.clientWidth + 1 ? clippedText(el, "x") : null;
      if (cutX !== null) {
        failures.push({
          kind: "clipped-x",
          selector: describe(el),
          text: cutX,
          detail: `scrollWidth ${el.scrollWidth} > clientWidth ${el.clientWidth}`,
        });
      }
      const cutY = clipY && el.scrollHeight > el.clientHeight + 1 ? clippedText(el, "y") : null;
      if (cutY !== null) {
        failures.push({
          kind: "clipped-y",
          selector: describe(el),
          text: cutY,
          detail: `scrollHeight ${el.scrollHeight} > clientHeight ${el.clientHeight}`,
        });
      }
    }

    // A button whose own text spills out of it, even where nothing clips it
    // (it then runs into its neighbours or past its column).
    if (!truncates && el.tagName === "BUTTON" && !clipX && el.scrollWidth > el.clientWidth + 1) {
      const spill = clippedText(el, "x");
      if (spill !== null) {
        failures.push({
          kind: "button-spills",
          selector: describe(el),
          text: spill,
          detail: `scrollWidth ${el.scrollWidth} > clientWidth ${el.clientWidth}`,
        });
      }
    }

    // A button wider than the box it sits in (its words do not fit, so it
    // grows past its column).
    if (!truncates && el.tagName === "BUTTON" && el.parentElement) {
      // The nearest ancestor with a box (display: contents has none).
      let parent = el.parentElement;
      while (parent.parentElement && getComputedStyle(parent).display === "contents") parent = parent.parentElement;
      const ps = getComputedStyle(parent);
      const pr = parent.getBoundingClientRect();
      const inner = pr.left + parent.clientLeft + parent.clientWidth;
      if (ps.overflowX === "visible" && style.position !== "absolute" && style.position !== "fixed" && rect.right > inner + 1) {
        failures.push({
          kind: "button-outgrows-parent",
          selector: describe(el),
          text: snippet(el),
          detail: `right ${Math.round(rect.right)} > parent's ${Math.round(inner)}`,
        });
      }
    }

    // Sticking out of the viewport on the right (or left).
    if (!truncates && (rect.right > vw + 1 || rect.left < -1) && style.position !== "fixed") {
      // Only count it when nothing between it and the page clips it away.
      let clipped = false;
      for (let p = el.parentElement; p && p !== document.documentElement; p = p.parentElement) {
        const ps = getComputedStyle(p);
        if (ps.overflowX !== "visible") {
          const pr = p.getBoundingClientRect();
          if (pr.right <= vw + 1 && pr.left >= -1) clipped = true;
          break;
        }
      }
      if (!clipped) {
        failures.push({
          kind: "outside-viewport",
          selector: describe(el),
          text: snippet(el),
          detail: `left ${Math.round(rect.left)} right ${Math.round(rect.right)} > viewport ${vw}`,
        });
      }
    }
  }

  const se = document.scrollingElement || document.documentElement;
  if (se.scrollWidth > vw + 1) {
    failures.push({ kind: "page-scrolls-x", selector: "document", text: "", detail: `scrollWidth ${se.scrollWidth} > ${vw}` });
  }
  return failures;
}

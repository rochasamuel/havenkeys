// Stricter visibility for identity fields (final review finding 1).
//
// An identity fill has no site binding, so a page could otherwise place
// phone, address or birth-date inputs where the user cannot see them and
// collect the values with one click. `isRendered` (group.ts) is enough for
// logins, which Rust binds to the site; identity fields must also pass this.
//
// Checked on top of isRendered (group.ts defaultEnv combines the two), all
// bounded (MAX_ANCESTORS elements, no page-wide work):
// * the box lies inside the document's scrollable area (no -9999px);
// * the combined opacity of the field and its ancestors is at least 0.1;
// * no clip / clip-path on the field or an ancestor collapses it;
// * no ancestor with overflow hidden/clip is collapsed or cuts the field off.
//
// Not checked: a field covered by another element. Hit testing would refuse
// forms whose floating labels sit over their inputs (documented in
// docs/security-model.md as a known limitation).

/** Ancestors examined, the field included. */
export const MAX_ANCESTORS = 16;
/** Below this combined opacity a field counts as invisible. */
export const MIN_OPACITY = 0.1;
/** Smallest visible extent, as in isRendered. */
const MIN_SIZE = 4;

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Layout reads, injectable because jsdom has no layout. */
export interface Layout {
  rect(el: Element): Box;
  style(el: Element): { opacity: string; overflowX: string; overflowY: string; clip: string; clipPath: string; position: string };
  /** Scroll offset and the document's scrollable size (0 when unknown). */
  page(): { scrollX: number; scrollY: number; width: number; height: number };
}

export const domLayout: Layout = {
  rect: (el) => el.getBoundingClientRect(),
  style: (el) => getComputedStyle(el),
  page: () => {
    const d = document.documentElement;
    return {
      scrollX: window.scrollX,
      scrollY: window.scrollY,
      width: Math.max(d.scrollWidth, d.clientWidth),
      height: Math.max(d.scrollHeight, d.clientHeight),
    };
  },
};

/** A CSS length in px (or % of `base`); NaN for anything else. */
function length(raw: string, base: number): number {
  const s = raw.trim();
  if (s === "0") return 0;
  const m = /^(-?\d*\.?\d+)(px|%)$/.exec(s);
  if (!m) return Number.NaN;
  const n = Number(m[1]);
  return m[2] === "%" ? (n / 100) * base : n;
}

/** Does `clip-path` leave less than MIN_SIZE of a `w`×`h` box? Unknown shapes count as visible. */
function clipPathCollapses(value: string, w: number, h: number): boolean {
  const v = value.trim().toLowerCase();
  if (v === "" || v === "none") return false;
  const fn = /^(inset|circle|ellipse|polygon)\((.*)\)/.exec(v);
  if (!fn) return false;
  const args = (fn[2] ?? "").split(/\s+round\s+/)[0] ?? "";
  if (fn[1] === "inset") {
    const p = args.trim().split(/\s+/).slice(0, 4);
    const [t = "0", r = t, b = t, l = r] = p;
    const vert = length(t, h) + length(b, h);
    const horiz = length(l, w) + length(r, w);
    return h - vert < MIN_SIZE || w - horiz < MIN_SIZE;
  }
  if (fn[1] === "circle" || fn[1] === "ellipse") {
    const radii = (args.split(/\s+at\s+/)[0] ?? "").trim().split(/\s+/).filter(Boolean);
    if (radii.length === 0) return false;
    const base = Math.min(w, h);
    return radii.some((r) => length(r, base) * 2 < MIN_SIZE);
  }
  // polygon: the bounding box of its points.
  const points = args.replace(/^\s*(nonzero|evenodd)\s*,/, "").split(",").slice(0, 64);
  const xs: number[] = [];
  const ys: number[] = [];
  for (const pt of points) {
    const [x = "", y = ""] = pt.trim().split(/\s+/);
    const px = length(x, w);
    const py = length(y, h);
    if (Number.isNaN(px) || Number.isNaN(py)) return false;
    xs.push(px);
    ys.push(py);
  }
  if (xs.length === 0) return false;
  return Math.max(...xs) - Math.min(...xs) < MIN_SIZE || Math.max(...ys) - Math.min(...ys) < MIN_SIZE;
}

/** Does legacy `clip: rect(...)` (absolute/fixed only) leave less than MIN_SIZE? */
function clipCollapses(value: string, position: string, w: number, h: number): boolean {
  if (position !== "absolute" && position !== "fixed") return false;
  const m = /^rect\((.*)\)$/.exec(value.trim().toLowerCase());
  if (!m) return false;
  const parts = (m[1] ?? "").split(/[\s,]+/).filter(Boolean);
  if (parts.length !== 4) return false;
  const [t, r, b, l] = parts.map((p, i) => (p === "auto" ? (i === 1 ? w : i === 2 ? h : 0) : length(p, 0))) as [number, number, number, number];
  if ([t, r, b, l].some(Number.isNaN)) return false;
  return r - l < MIN_SIZE || b - t < MIN_SIZE;
}

const clipsOverflow = (v: string) => v === "hidden" || v === "clip";

/** Not hidden by the tricks above. Callers also require isRendered (see defaultEnv). */
export function isIdentityVisible(el: HTMLElement, layout: Layout = domLayout): boolean {
  const box = layout.rect(el);
  const page = layout.page();
  // Outside the scrollable area: no scrolling brings it into view.
  const left = box.left + page.scrollX;
  const top = box.top + page.scrollY;
  if (left + box.width <= 0 || top + box.height <= 0) return false;
  if (page.width > 0 && left >= page.width) return false;
  if (page.height > 0 && top >= page.height) return false;

  let opacity = 1;
  let node: Element | null = el;
  for (let i = 0; node && i < MAX_ANCESTORS; i++, node = node.parentElement) {
    const s = layout.style(node);
    const o = Number.parseFloat(s.opacity);
    if (!Number.isNaN(o)) opacity *= o;
    if (opacity < MIN_OPACITY) return false;
    const clipPath = s.clipPath ?? "";
    const clip = s.clip ?? "";
    const overflows = node !== el && (clipsOverflow(s.overflowX) || clipsOverflow(s.overflowY));
    const clipped = (clipPath !== "" && clipPath !== "none") || (clip !== "" && clip !== "auto");
    if (!clipped && !overflows) continue;
    // Only read an ancestor's box when something on it can cut the field.
    const r = node === el ? box : layout.rect(node);
    if (clipPathCollapses(clipPath, r.width, r.height)) return false;
    if (clipCollapses(clip, s.position ?? "", r.width, r.height)) return false;
    if (overflows) {
      if (r.width < MIN_SIZE || r.height < MIN_SIZE) return false;
      // The part of the field this ancestor lets through.
      const w = Math.min(box.left + box.width, r.left + r.width) - Math.max(box.left, r.left);
      const h = Math.min(box.top + box.height, r.top + r.height) - Math.max(box.top, r.top);
      if ((clipsOverflow(s.overflowX) && w < MIN_SIZE) || (clipsOverflow(s.overflowY) && h < MIN_SIZE)) return false;
    }
  }
  return true;
}

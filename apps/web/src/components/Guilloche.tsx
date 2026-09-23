/*
 * The engraved rosette behind the hero: the pattern banknotes and share
 * certificates use because it is hard to reproduce. Computed once from a
 * family of hypotrochoids, so it is a few KB of path data, not an image.
 */
function hypotrochoid(R: number, r: number, d: number, scale: number, turns: number, steps: number): string {
  const k = (R - r) / r;
  let out = "";
  for (let i = 0; i <= steps; i++) {
    const t = (i / steps) * Math.PI * 2 * turns;
    const x = ((R - r) * Math.cos(t) + d * Math.cos(k * t)) * scale;
    const y = ((R - r) * Math.sin(t) - d * Math.sin(k * t)) * scale;
    out += `${i === 0 ? "M" : "L"}${x.toFixed(1)} ${y.toFixed(1)}`;
  }
  return out;
}

const RINGS = [
  hypotrochoid(96, 36, 58, 4.6, 3, 1400),
  hypotrochoid(96, 36, 44, 4.1, 3, 1400),
  hypotrochoid(105, 21, 40, 3.6, 1, 1100),
  hypotrochoid(80, 30, 52, 3.2, 3, 1200),
  hypotrochoid(90, 18, 26, 2.3, 1, 900),
];

export function Guilloche({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="-600 -600 1200 1200" aria-hidden="true" focusable="false">
      <g fill="none" stroke="currentColor" strokeWidth="0.9">
        {RINGS.map((d, i) => (
          <path key={i} d={d} opacity={1 - i * 0.12} />
        ))}
      </g>
    </svg>
  );
}

import { useI18n } from "../i18n/context";

/*
 * An illustration of the printed Emergency Kit. The key is made up and the
 * code is decorative (it encodes nothing); the page says so on the sheet.
 */
function qrCells(): boolean[] {
  const n = 25;
  const cells: boolean[] = [];
  let seed = 0x9e3779b9;
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      const inFinder = (fx: number, fy: number) => x >= fx && x < fx + 7 && y >= fy && y < fy + 7;
      const finder = [
        [0, 0],
        [n - 7, 0],
        [0, n - 7],
      ].find(([fx, fy]) => inFinder(fx!, fy!));
      if (finder) {
        const dx = x - finder[0]!;
        const dy = y - finder[1]!;
        const ring = Math.max(Math.abs(dx - 3), Math.abs(dy - 3));
        cells.push(ring !== 2);
        continue;
      }
      seed = (seed * 1664525 + 1013904223) >>> 0;
      cells.push(seed % 100 < 46);
    }
  }
  return cells;
}

const CELLS = qrCells();

/* The sheet itself stays in English: it depicts the kit the app prints. */
export function EmergencyKit() {
  const { t } = useI18n();
  return (
    <figure className="kit" aria-label={t.kit.aria}>
      <div className="kit__sheet" lang="en">
        <header className="kit__head">
          <span className="kit__brand">HavenKeys</span>
          <span className="kit__doc">Emergency Kit</span>
        </header>
        <p className="kit__intro">
          Keep this page somewhere safe. With your master password, it lets you sign in on a new
          computer.
        </p>
        <div className="kit__key">
          <span className="kit__label">Secret Key</span>
          <span className="kit__value">
            H1-7QKP-2M9X-R4TD
            <br />
            8WVN-3CJF-6HLA-Y5ZE
          </span>
        </div>
        <div className="kit__row">
          <svg className="kit__qr" viewBox="0 0 25 25" aria-hidden="true" shapeRendering="crispEdges">
            {CELLS.map((on, i) =>
              on ? <rect key={i} x={i % 25} y={Math.floor(i / 25)} width="1" height="1" /> : null,
            )}
          </svg>
          <div className="kit__lines">
            <span className="kit__label">Master password</span>
            <span className="kit__blank" />
            <span className="kit__hint">Not printed. Only you know it.</span>
          </div>
        </div>
      </div>
      <figcaption>{t.kit.caption}</figcaption>
    </figure>
  );
}

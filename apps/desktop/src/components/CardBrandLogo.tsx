import { CARD_BRAND_ICONS, GENERIC_CARD_ICON } from "@havenkeys/ui/card-brand-icons";
import type { CardBrand } from "../lib/types";

/** The card network's mark, 780×500 like a card; the generic card for `other`. */
export function CardBrandLogo({ brand, width = 32 }: { brand: CardBrand; width?: number }) {
  const icon = brand === "other" ? GENERIC_CARD_ICON : CARD_BRAND_ICONS[brand];
  return (
    <svg className="card-logo" width={width} height={Math.round((width * 500) / 780)} viewBox={icon.viewBox} aria-hidden="true">
      {icon.shapes.map((s, i) =>
        s.rect ? <rect key={i} x={s.rect[0]} y={s.rect[1]} width={s.rect[2]} height={s.rect[3]} fill={s.fill} /> : <path key={i} d={s.d} fill={s.fill} />,
      )}
    </svg>
  );
}

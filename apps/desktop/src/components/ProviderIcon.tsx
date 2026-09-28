import { PROVIDER_ICONS } from "@havenkeys/ui/provider-icons";
import type { SsoProvider } from "../lib/types";

export function ProviderIcon({ provider, size = 18 }: { provider: SsoProvider; size?: number }) {
  const icon = PROVIDER_ICONS[provider];
  return (
    <svg className="provider-icon" width={size} height={size} viewBox={icon.viewBox} aria-hidden="true">
      {icon.shapes.map((s, i) =>
        s.rect ? <rect key={i} x={s.rect[0]} y={s.rect[1]} width={s.rect[2]} height={s.rect[3]} fill={s.fill} /> : <path key={i} d={s.d} fill={s.fill} />,
      )}
    </svg>
  );
}

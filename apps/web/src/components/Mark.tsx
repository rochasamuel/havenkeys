/** The HavenKeys shield and keyhole, drawn from apps/desktop/assets/icon.svg. */
export function Mark({ size = 28, tile = true }: { size?: number; tile?: boolean }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden="true" focusable="false">
      {tile && <rect width="1024" height="1024" rx="224" fill="#14201c" />}
      <path
        d="M512 196 L772 300 V500 C772 668 662 792 512 846 C362 792 252 668 252 500 V300 Z"
        fill="none"
        stroke="#e8c37a"
        strokeWidth="44"
        strokeLinejoin="round"
      />
      <circle cx="512" cy="468" r="84" fill="#e8c37a" />
      <rect x="486" y="520" width="52" height="170" rx="18" fill="#e8c37a" />
    </svg>
  );
}

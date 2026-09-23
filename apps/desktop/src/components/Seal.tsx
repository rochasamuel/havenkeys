/**
 * The HavenKeys mark: a shield with a keyhole, the same drawing as the app
 * icon and the website. `open` turns the key while the vault unlocks.
 */
export function Seal({ open = false, size = 64 }: { open?: boolean; size?: number }) {
  return (
    <svg
      className={`seal${open ? " seal-open" : ""}`}
      width={size}
      height={size}
      viewBox="0 0 1024 1024"
      aria-hidden="true"
    >
      <path
        className="seal-shield"
        d="M512 196 L772 300 V500 C772 668 662 792 512 846 C362 792 252 668 252 500 V300 Z"
      />
      <g className="seal-key">
        <circle cx="512" cy="468" r="84" />
        <rect x="486" y="520" width="52" height="170" rx="18" />
      </g>
    </svg>
  );
}

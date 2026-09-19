/** The brass shield mark. `open` lifts the shackle while unlocking. */
export function Seal({ open = false, size = 72 }: { open?: boolean; size?: number }) {
  return (
    <svg
      className={`seal${open ? " seal-open" : ""}`}
      width={size}
      height={size}
      viewBox="0 0 64 64"
      aria-hidden="true"
    >
      <path
        className="seal-shield"
        d="M32 5 L54 13.5 V30 C54 44 45 54.5 32 59 C19 54.5 10 44 10 30 V13.5 Z"
      />
      <path className="seal-shackle" d="M25 30 V24 a7 7 0 0 1 14 0 V30" />
      <rect className="seal-body" x="22" y="29" width="20" height="15" rx="3" />
    </svg>
  );
}

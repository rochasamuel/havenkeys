import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";

/**
 * A copy action that confirms itself: the icon becomes a check for a moment
 * after `onCopy` resolves true. The copy itself happens in Rust; this only
 * shows that it did.
 */
export function CopyButton({ label, onCopy }: { label: string; onCopy: () => Promise<boolean> }) {
  const [done, setDone] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  return (
    <button
      type="button"
      className={`icon-btn copy-btn${done ? " is-done" : ""}`}
      title={label}
      aria-label={label}
      onClick={async () => {
        if (await onCopy()) {
          setDone(true);
          window.clearTimeout(timer.current);
          timer.current = window.setTimeout(() => setDone(false), 1400);
        }
      }}
    >
      <span className="copy-glyph copy-glyph-copy">
        <Icon name="copy" size={16} />
      </span>
      <span className="copy-glyph copy-glyph-check">
        <Icon name="check" size={16} />
      </span>
    </button>
  );
}

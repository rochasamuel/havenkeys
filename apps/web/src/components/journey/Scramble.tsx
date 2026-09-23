import { useEffect, useRef, useState } from "react";

const HEX = "0123456789abcdef";

function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * Text that turns into its ciphertext: characters resolve left to right,
 * flickering through hex until they settle. Demo data only; nothing here is
 * real encryption, and it never needs to be.
 */
export function Scramble({ plain, cipher, sealed }: { plain: string; cipher: string; sealed: boolean }) {
  const [shown, setShown] = useState(sealed ? cipher : plain);
  const frame = useRef(0);

  useEffect(() => {
    const target = sealed ? cipher : plain;
    const from = sealed ? plain : cipher;
    if (prefersReducedMotion()) {
      setShown(target);
      return;
    }
    const start = performance.now();
    const duration = 900;
    const tick = (now: number) => {
      const p = Math.min(1, (now - start) / duration);
      const len = Math.round(from.length + (target.length - from.length) * p);
      let out = "";
      for (let i = 0; i < len; i++) {
        const settle = i / Math.max(len, 1);
        if (p >= 1 || p > settle + 0.18) out += target[i] ?? "";
        else if (p > settle - 0.1) out += HEX[Math.floor(Math.random() * 16)];
        else out += from[i] ?? HEX[Math.floor(Math.random() * 16)];
      }
      setShown(out);
      if (p < 1) frame.current = requestAnimationFrame(tick);
    };
    frame.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame.current);
  }, [sealed, plain, cipher]);

  return <>{shown}</>;
}

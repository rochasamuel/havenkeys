import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import type { TotpCode } from "./types";

const ACTIVITY_THROTTLE_MS = 20_000;

/** Report user activity to the core's auto-lock timer (throttled). */
export function useActivityReporter(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    let last = 0;
    const onActivity = () => {
      // A mouse drifting over a background window is not the user working.
      if (!document.hasFocus()) return;
      const now = Date.now();
      if (now - last < ACTIVITY_THROTTLE_MS) return;
      last = now;
      void api.recordActivity().catch(() => undefined);
    };
    const events = ["pointerdown", "keydown", "wheel", "pointermove"] as const;
    events.forEach((e) => window.addEventListener(e, onActivity, { passive: true }));
    return () => events.forEach((e) => window.removeEventListener(e, onActivity));
  }, [enabled]);
}

/**
 * A secret shown on explicit request. Hidden again after `autoHideMs`, when
 * `hide()` is called, or when the component unmounts (e.g. on lock).
 */
export function useRevealedSecret(load: () => Promise<string>, autoHideMs = 30_000) {
  const [value, setValue] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const hide = useCallback(() => {
    window.clearTimeout(timer.current);
    setValue(null);
  }, []);

  const reveal = useCallback(async () => {
    const v = await load();
    setValue(v);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setValue(null), autoHideMs);
  }, [load, autoHideMs]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return { value, reveal, hide };
}

/** Live TOTP code for an item; refreshes at the period boundary. */
export function useTotp(itemId: string, enabled: boolean) {
  const [code, setCode] = useState<TotpCode | null>(null);
  const [remaining, setRemaining] = useState(0);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let deadline = 0;
    let tick: number | undefined;

    const fetchCode = async () => {
      try {
        const c = await api.totp(itemId);
        if (cancelled) return;
        deadline = Date.now() + c.secondsRemaining * 1000;
        setCode(c);
        setRemaining(c.secondsRemaining);
        setFailed(false);
      } catch {
        if (!cancelled) setFailed(true);
      }
    };

    void fetchCode();
    tick = window.setInterval(() => {
      const left = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
      setRemaining(left);
      if (left === 0 && deadline !== 0) {
        deadline = 0;
        void fetchCode();
      }
    }, 250);

    return () => {
      cancelled = true;
      window.clearInterval(tick);
      setCode(null);
    };
  }, [itemId, enabled]);

  return { code, remaining, failed };
}

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./api";
import type { TotpCode, UpdateStatus } from "./types";

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

/** A live TOTP code; refreshes at the period boundary. `key` restarts it. */
export function useTotp(load: () => Promise<TotpCode>, key: string, enabled: boolean) {
  const [code, setCode] = useState<TotpCode | null>(null);
  const [remaining, setRemaining] = useState(0);
  const [failed, setFailed] = useState(false);
  const loadRef = useRef(load);
  loadRef.current = load;

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let deadline = 0;
    let tick: number | undefined;

    const fetchCode = async () => {
      try {
        const c = await loadRef.current();
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
  }, [key, enabled]);

  return { code, remaining, failed };
}

/** The in-app update status, kept current by Rust's `updates://status` event. */
export function useUpdateStatus(): UpdateStatus | null {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  useEffect(() => {
    let live = true;
    api.updateStatus().then(
      (s) => {
        if (live) setStatus(s);
      },
      () => undefined,
    );
    const unlisten = api.onUpdateStatus((s) => setStatus(s));
    return () => {
      live = false;
      void unlisten.then((f) => f());
    };
  }, []);
  return status;
}

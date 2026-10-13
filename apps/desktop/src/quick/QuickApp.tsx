import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import { useActivityReporter } from "../lib/hooks";
import { QuickSearch } from "./QuickSearch";
import { QuickUnlock } from "./QuickUnlock";

type View = "loading" | "locked" | "unlocked";

/**
 * The Quick Access window. Holds no secret: a master password lives only in
 * QuickUnlock's field until it is sent, and the search holds overviews.
 * Every show and every lock bumps `session`, which remounts the view and
 * so drops the query, the selection and anything typed.
 */
export function QuickApp() {
  const [view, setView] = useState<View>("loading");
  const [session, setSession] = useState(0);

  const refresh = useCallback(() => {
    api.status().then(
      (s) => setView(s.state === "unlocked" ? "unlocked" : "locked"),
      () => setView("locked"),
    );
  }, []);

  useEffect(() => {
    refresh();
    const offShown = api.onQuickShown(() => {
      setSession((s) => s + 1);
      refresh();
    });
    const offLocked = api.onLocked(() => {
      setSession((s) => s + 1);
      setView("locked");
    });
    // An unlock elsewhere (main window, phone sign-in) moves this window on.
    const offUnlocked = api.onUnlocked(() => refresh());
    return () => {
      void offUnlocked.then((f) => f());
      void offShown.then((f) => f());
      void offLocked.then((f) => f());
    };
  }, [refresh]);

  // Typing and clicking here count as activity, as in the main window.
  useActivityReporter(view === "unlocked");

  const hide = useCallback(() => void api.hideQuickWindow().catch(() => undefined), []);

  if (view === "loading") return <main className="quick" aria-busy="true" />;
  if (view === "locked") return <QuickUnlock key={session} onUnlocked={() => setView("unlocked")} onHide={hide} />;
  return <QuickSearch key={session} onHide={hide} />;
}

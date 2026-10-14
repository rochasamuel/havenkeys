import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../lib/api";
import type { UpdateConfirmation } from "../lib/types";
import { useI18n } from "../i18n/context";

/** How long Allow stays disabled after the dialog opens. */
const ARM_DELAY_MS = 500;

/**
 * "Change this password?" The desktop's Allow or Deny for a browser
 * password change of a login whose password already changed in the last 24
 * hours (spec 2026-10-13-confirm-repeat-password-change §5). Shows the
 * login's title and username and the page's site as text, never a password.
 * Deny is the default (primary, first, focused) and the Escape action;
 * Allow is enabled only after a short delay. Closes once answered, on lock,
 * and when Rust says the wait ended (timeout, window closed). Rust's reply to
 * an answer is not shown: a late answer may be accepted while the browser was
 * already denied, so the dialog never claims the change happened.
 */
export function UpdateConfirmDialog() {
  const { t } = useI18n();
  const [pending, setPending] = useState<UpdateConfirmation | null>(null);
  // Allow stays off for a moment after the dialog opens, so a click already
  // on its way (meant for the screen behind) cannot answer it.
  // Keyed to the confirmation's ID, so a new one is never born armed.
  const [armedId, setArmedId] = useState<string | null>(null);
  const denyRef = useRef<HTMLButtonElement>(null);
  const pendingRef = useRef<UpdateConfirmation | null>(null);
  pendingRef.current = pending;

  useEffect(() => {
    const offAsk = api.onConfirmUpdate(setPending);
    const offClosed = api.onConfirmUpdateClosed((id) => setPending((p) => (p?.id === id ? null : p)));
    const offLocked = api.onLocked(() => setPending(null));
    return () => {
      void offAsk.then((f) => f());
      void offClosed.then((f) => f());
      void offLocked.then((f) => f());
    };
  }, []);

  // The one way to answer: the buttons and Escape all come here.
  const answer = useCallback((allow: boolean) => {
    const current = pendingRef.current;
    if (!current) return;
    pendingRef.current = null;
    setPending(null);
    // Refused only when Rust already ended this wait; nothing to show.
    void api.answerUpdateConfirmation(current.id, allow).catch(() => undefined);
  }, []);

  const pendingId = pending?.id ?? null;

  // While open, the app behind it is inert, Deny has the focus, and Escape
  // anywhere in the window (focus can fall to the body) answers Deny. On
  // close, the focus goes back where it was.
  useEffect(() => {
    if (!pendingId) return;
    const appRoot = document.getElementById("root");
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    appRoot?.setAttribute("inert", "");
    denyRef.current?.focus();
    const arm = window.setTimeout(() => setArmedId(pendingId), ARM_DELAY_MS);
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      answer(false);
    };
    document.addEventListener("keydown", onKey, true);
    return () => {
      window.clearTimeout(arm);
      document.removeEventListener("keydown", onKey, true);
      appRoot?.removeAttribute("inert");
      if (previous?.isConnected) previous.focus();
    };
  }, [pendingId, answer]);

  if (!pending) return null;
  const armed = armedId === pending.id;

  return createPortal(
    <div className="modal-scrim">
      <div
        className="modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-update-title"
        aria-describedby="confirm-update-body"
      >
        <h2 id="confirm-update-title" className="modal-title">
          {t.confirmUpdate.title}
        </h2>
        <p id="confirm-update-body" className="modal-body">
          {t.confirmUpdate.body(pending.title, pending.username, pending.site)}
        </p>
        <div className="modal-actions">
          <button ref={denyRef} className="btn btn-primary" type="button" onClick={() => answer(false)}>
            {t.confirmUpdate.deny}
          </button>
          <button className="btn" type="button" disabled={!armed} onClick={() => answer(true)}>
            {t.confirmUpdate.allow}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { api, ApiError } from "../lib/api";
import { Icon } from "../components/Icon";
import { Seal } from "../components/Seal";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

interface Props {
  onUnlocked: () => void;
  onHide: () => void;
}

/** The master password, typed here and sent to unlock_vault. Never a Secret Key: that is entered in the main window. */
export function QuickUnlock({ onUnlocked, onHide }: Props) {
  const { t } = useI18n();
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Bumped on every failure so the shake replays, as on the unlock screen.
  const [attempt, setAttempt] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // The field remounts with each attempt: focus the new one.
  useEffect(() => inputRef.current?.focus(), [attempt]);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !password) return;
    setBusy(true);
    setError(null);
    try {
      await api.unlock(password);
      setPassword("");
      onUnlocked();
    } catch (err) {
      setError(
        err instanceof ApiError && err.code === "secret_key_required"
          ? t.quick.secretKeyNeeded
          : errorMessage(err, t, t.unlock.failed),
      );
      setPassword("");
      setBusy(false);
      setAttempt((a) => a + 1);
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onHide();
    }
  }

  return (
    <main className="quick quick-locked" onKeyDown={onKeyDown}>
      <form className="quick-unlock" onSubmit={submit} aria-busy={busy}>
        <Seal open={busy} size={40} />
        <p className="quick-lede">{t.quick.unlockLede}</p>
        <div key={attempt} className={`unlock-fields${attempt > 0 ? " is-shaking" : ""}`}>
          <label className="unlock-field">
            <span className="sr-only">{t.common.masterPassword}</span>
            <input
              ref={inputRef}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              placeholder={t.common.masterPassword}
              disabled={busy}
              aria-invalid={!!error}
            />
            <button className="unlock-go" type="submit" disabled={busy || !password} aria-label={t.unlock.unlock}>
              {busy ? <span className="spinner" aria-hidden="true" /> : <Icon name="arrowRight" size={18} />}
            </button>
          </label>
        </div>
        <p className={`unlock-error${error ? " is-visible" : ""}`} role="alert">
          {error}
        </p>
      </form>
    </main>
  );
}

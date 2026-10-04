import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { VaultStatus } from "../lib/types";
import { Guilloche } from "../components/Guilloche";
import { Icon } from "../components/Icon";
import { Seal } from "../components/Seal";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

interface Props {
  lockReason: string | null;
  /** The vault needs a Secret Key this device does not have yet. */
  needsSecretKey: boolean;
  onUnlocked: (status: VaultStatus) => void;
}

export function UnlockScreen({ lockReason, needsSecretKey, onUnlocked }: Props) {
  const { t } = useI18n();
  const [password, setPassword] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Bumped on every failure so the shake animation replays.
  const [attempt, setAttempt] = useState(0);
  const [askKey, setAskKey] = useState(needsSecretKey);
  // The keychain did not answer (busy, a prompt waiting, or failing). The
  // Secret Key field is not opened for that: the key is probably there, and
  // one typed now would be saved to a file. Only if the user asks for it.
  const [keychainStuck, setKeychainStuck] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => inputRef.current?.focus(), []);
  // Raised from the tray or the extension's "Unlock": the password field
  // takes the keyboard again, unless another field already has it.
  useEffect(() => {
    const refocus = () => {
      const active = document.activeElement;
      if (!active || active === document.body) inputRef.current?.focus();
    };
    window.addEventListener("focus", refocus);
    return () => window.removeEventListener("focus", refocus);
  }, []);

  const ready = !!password && (!askKey || !!secretKey);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !ready) return;
    setBusy(true);
    setError(null);
    try {
      const status = await api.unlock(password, askKey ? secretKey : undefined);
      setPassword("");
      setSecretKey("");
      onUnlocked(status);
    } catch (err) {
      if (err instanceof ApiError && err.code === "secret_key_required") setAskKey(true);
      setKeychainStuck(err instanceof ApiError && err.code === "keychain_unavailable");
      setError(errorMessage(err, t, t.unlock.failed));
      setAttempt((n) => n + 1);
      setPassword("");
      setBusy(false);
      inputRef.current?.focus();
    }
  }

  return (
    <main className="unlock" data-tauri-drag-region>
      <Guilloche className="unlock-rosette" />
      <form className="unlock-panel" onSubmit={submit} aria-busy={busy}>
        <Seal open={busy} size={72} />
        <h1 className="unlock-title">
          {t.unlock.titleBefore}
          <em>{t.unlock.titleLocked}</em>
          {t.unlock.titleAfter}
        </h1>
        <p className="unlock-lede">
          {askKey
            ? t.unlock.enterPasswordAndKey
            : ((lockReason && t.unlock.reasons[lockReason]) ?? t.unlock.enterPassword)}
        </p>

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
            {!askKey && (
              <button className="unlock-go" type="submit" disabled={busy || !ready} aria-label={t.unlock.unlock}>
                {busy ? <span className="spinner" aria-hidden="true" /> : <Icon name="arrowRight" size={18} />}
              </button>
            )}
          </label>

          {askKey && (
            <label className="unlock-field unlock-field-key">
              <span className="sr-only">{t.common.secretKey}</span>
              <input
                className="mono"
                type="text"
                value={secretKey}
                onChange={(e) => setSecretKey(e.target.value)}
                autoComplete="off"
                spellCheck={false}
                autoCapitalize="off"
                placeholder={t.unlock.secretKeyPlaceholder}
                disabled={busy}
              />
              <button className="unlock-go" type="submit" disabled={busy || !ready} aria-label={t.unlock.unlock}>
                {busy ? <span className="spinner" aria-hidden="true" /> : <Icon name="arrowRight" size={18} />}
              </button>
            </label>
          )}
        </div>

        <p className={`unlock-error${error ? " is-visible" : ""}`} role="alert">
          {error}
        </p>
        {keychainStuck && !askKey && (
          <button className="btn btn-quiet" type="button" onClick={() => setAskKey(true)} disabled={busy}>
            {t.unlock.useKitInstead}
          </button>
        )}
      </form>
      <p className="unlock-hint">
        <Icon name="shield" size={13} /> {t.unlock.hint}
      </p>
    </main>
  );
}

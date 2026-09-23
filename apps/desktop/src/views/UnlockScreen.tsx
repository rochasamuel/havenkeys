import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { VaultStatus } from "../lib/types";
import { Guilloche } from "../components/Guilloche";
import { Icon } from "../components/Icon";
import { Seal } from "../components/Seal";

interface Props {
  lockReason: string | null;
  /** The vault needs a Secret Key this device does not have yet. */
  needsSecretKey: boolean;
  onUnlocked: (status: VaultStatus) => void;
}

const reasonText: Record<string, string> = {
  idle: "Locked after a period of inactivity.",
  suspend: "Locked because the computer went to sleep.",
  screen_lock: "Locked because the screen was locked.",
  user: "Locked.",
  extension: "Locked from the browser extension.",
};

export function UnlockScreen({ lockReason, needsSecretKey, onUnlocked }: Props) {
  const [password, setPassword] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Bumped on every failure so the shake animation replays.
  const [attempt, setAttempt] = useState(0);
  const [askKey, setAskKey] = useState(needsSecretKey);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => inputRef.current?.focus(), []);

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
      setError(err instanceof ApiError ? err.message : "Could not open the vault.");
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
          HavenKeys is <em>locked</em>.
        </h1>
        <p className="unlock-lede">
          {askKey
            ? "Enter your master password and the Secret Key from your Emergency Kit. This computer remembers the Secret Key after you unlock."
            : ((lockReason && reasonText[lockReason]) ?? "Enter your master password to unlock.")}
        </p>

        <div key={attempt} className={`unlock-fields${attempt > 0 ? " is-shaking" : ""}`}>
          <label className="unlock-field">
            <span className="sr-only">Master password</span>
            <input
              ref={inputRef}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              placeholder="Master password"
              disabled={busy}
              aria-invalid={!!error}
            />
            {!askKey && (
              <button className="unlock-go" type="submit" disabled={busy || !ready} aria-label="Unlock">
                {busy ? <span className="spinner" aria-hidden="true" /> : <Icon name="arrowRight" size={18} />}
              </button>
            )}
          </label>

          {askKey && (
            <label className="unlock-field unlock-field-key">
              <span className="sr-only">Secret Key</span>
              <input
                className="mono"
                type="text"
                value={secretKey}
                onChange={(e) => setSecretKey(e.target.value)}
                autoComplete="off"
                spellCheck={false}
                autoCapitalize="off"
                placeholder="Secret Key  H1-XXXX-XXXX-…"
                disabled={busy}
              />
              <button className="unlock-go" type="submit" disabled={busy || !ready} aria-label="Unlock">
                {busy ? <span className="spinner" aria-hidden="true" /> : <Icon name="arrowRight" size={18} />}
              </button>
            </label>
          )}
        </div>

        <p className={`unlock-error${error ? " is-visible" : ""}`} role="alert">
          {error}
        </p>
      </form>
      <p className="unlock-hint">
        <Icon name="shield" size={13} /> Your vault is decrypted on this computer only.
      </p>
    </main>
  );
}

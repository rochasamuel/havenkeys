import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { VaultStatus } from "../lib/types";
import { Seal } from "../components/Seal";

const MIN_LENGTH = 10;

interface Props {
  mode: "create" | "unlock";
  lockReason: string | null;
  onUnlocked: (status: VaultStatus) => void;
}

const reasonText: Record<string, string> = {
  idle: "Locked after a period of inactivity.",
  suspend: "Locked because the computer went to sleep.",
  user: "Locked.",
  extension: "Locked from the browser extension.",
};

export function UnlockScreen({ mode, lockReason, onUnlocked }: Props) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => inputRef.current?.focus(), []);

  const creating = mode === "create";
  const tooShort = creating && password.length > 0 && [...password].length < MIN_LENGTH;
  const mismatch = creating && confirm.length > 0 && confirm !== password;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !password) return;
    if (creating && ([...password].length < MIN_LENGTH || password !== confirm)) return;
    setBusy(true);
    setError(null);
    try {
      const status = creating ? await api.createVault(password) : await api.unlock(password);
      setPassword("");
      setConfirm("");
      onUnlocked(status);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not open the vault.");
      setPassword("");
      setBusy(false);
      inputRef.current?.focus();
    }
  }

  return (
    <main className="unlock">
      <form className="unlock-panel" onSubmit={submit} aria-busy={busy}>
        <Seal open={busy} />
        <h1 className="unlock-title">{creating ? "Create your vault" : "HavenKeys is locked"}</h1>
        <p className="unlock-lede">
          {creating
            ? "Choose a master password. It encrypts everything in the vault and cannot be recovered if you forget it."
            : (lockReason && reasonText[lockReason]) ?? "Enter your master password to unlock."}
        </p>

        <label className="unlock-field">
          <span>Master password</span>
          <input
            ref={inputRef}
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="off"
            spellCheck={false}
            autoCapitalize="off"
            disabled={busy}
            aria-invalid={!!error || tooShort}
          />
        </label>
        {tooShort && <p className="unlock-hint">Use at least {MIN_LENGTH} characters. A few unrelated words work well.</p>}

        {creating && (
          <label className="unlock-field">
            <span>Confirm master password</span>
            <input
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              disabled={busy}
              aria-invalid={mismatch}
            />
          </label>
        )}
        {mismatch && <p className="unlock-hint">The passwords don’t match.</p>}

        {error && (
          <p className="unlock-error" role="alert">
            {error}
          </p>
        )}

        <button className="btn btn-brass unlock-submit" type="submit" disabled={busy || !password}>
          {busy ? (creating ? "Creating vault…" : "Unlocking…") : creating ? "Create vault" : "Unlock"}
        </button>

        {creating && (
          <p className="unlock-footnote">
            Your vault stays on this computer. Nothing is sent anywhere.
          </p>
        )}
      </form>
    </main>
  );
}

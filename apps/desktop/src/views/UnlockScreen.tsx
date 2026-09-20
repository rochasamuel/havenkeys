import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { VaultStatus } from "../lib/types";
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

function PasswordField(props: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  disabled: boolean;
  invalid?: boolean;
  inputRef?: React.Ref<HTMLInputElement>;
  mono?: boolean;
  placeholder?: string;
}) {
  return (
    <label className="unlock-field">
      <span>{props.label}</span>
      <input
        ref={props.inputRef}
        type={props.mono ? "text" : "password"}
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        autoCapitalize="off"
        placeholder={props.placeholder}
        disabled={props.disabled}
        aria-invalid={props.invalid}
      />
    </label>
  );
}

export function UnlockScreen({ lockReason, needsSecretKey, onUnlocked }: Props) {
  const [password, setPassword] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [askKey, setAskKey] = useState(needsSecretKey);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => inputRef.current?.focus(), []);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !password) return;
    if (askKey && !secretKey) return;
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
      setPassword("");
      setBusy(false);
      inputRef.current?.focus();
    }
  }

  return (
    <main className="unlock">
      <form className="unlock-panel" onSubmit={submit} aria-busy={busy}>
        <Seal open={busy} />
        <h1 className="unlock-title">HavenKeys is locked</h1>
        <p className="unlock-lede">
          {askKey
            ? "Enter your master password and the Secret Key from your Emergency Kit. The Secret Key is remembered on this computer after you unlock."
            : ((lockReason && reasonText[lockReason]) ?? "Enter your master password to unlock.")}
        </p>

        <PasswordField
          label="Master password"
          value={password}
          onChange={setPassword}
          disabled={busy}
          invalid={!!error}
          inputRef={inputRef}
        />

        {askKey && (
          <PasswordField
            label="Secret Key"
            value={secretKey}
            onChange={setSecretKey}
            disabled={busy}
            mono
            placeholder="H1-XXXX-XXXX-…"
          />
        )}

        {error && (
          <p className="unlock-error" role="alert">
            {error}
          </p>
        )}

        <button
          className="btn btn-brass unlock-submit"
          type="submit"
          disabled={busy || !password || (askKey && !secretKey)}
        >
          {busy ? "Unlocking…" : "Unlock"}
        </button>
      </form>
    </main>
  );
}

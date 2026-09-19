import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { VaultStatus } from "../lib/types";
import { Seal } from "../components/Seal";

const MIN_LENGTH = 10;

interface Props {
  mode: "create" | "unlock";
  lockReason: string | null;
  /** The vault needs a Secret Key this device does not have yet. */
  needsSecretKey: boolean;
  /** The vault uses a Secret Key at all (offer to type it in). */
  usesSecretKey: boolean;
  /** `created`: a brand-new vault, so the Emergency Kit comes next. */
  onUnlocked: (status: VaultStatus, created: boolean) => void;
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

/** New device: open the vault from a sync folder with password + Secret Key. */
function JoinPanel({ onBack, onJoined }: { onBack: () => void; onJoined: (s: VaultStatus) => void }) {
  const [folder, setFolder] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function choose() {
    setError(null);
    try {
      const picked = await api.pickJoinFolder();
      if (picked) setFolder(picked.folder);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not open that folder.");
    }
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !folder || !password || !secretKey) return;
    setBusy(true);
    setError(null);
    try {
      const status = await api.joinSyncedVault(password, secretKey);
      setPassword("");
      setSecretKey("");
      onJoined(status);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not open the vault.");
      setPassword("");
      setBusy(false);
    }
  }

  return (
    <form className="unlock-panel" onSubmit={submit} aria-busy={busy}>
      <Seal open={busy} />
      <h1 className="unlock-title">Set up from a sync folder</h1>
      <p className="unlock-lede">
        Use this if HavenKeys already runs on another computer that syncs through OneDrive, Dropbox, Google Drive or
        Syncthing. You need your master password and the Secret Key from your Emergency Kit.
      </p>
      <button className="btn unlock-submit" type="button" onClick={() => void choose()} disabled={busy}>
        {folder ? `Folder: ${folder}` : "Choose the sync folder…"}
      </button>
      {folder && (
        <>
          <PasswordField label="Master password" value={password} onChange={setPassword} disabled={busy} />
          <PasswordField
            label="Secret Key"
            value={secretKey}
            onChange={setSecretKey}
            disabled={busy}
            mono
            placeholder="H1-XXXX-XXXX-…"
          />
        </>
      )}
      {error && (
        <p className="unlock-error" role="alert">
          {error}
        </p>
      )}
      {folder && (
        <button className="btn btn-brass unlock-submit" type="submit" disabled={busy || !password || !secretKey}>
          {busy ? "Opening vault…" : "Open vault"}
        </button>
      )}
      <button className="btn btn-ghost unlock-submit" type="button" onClick={onBack} disabled={busy}>
        Back
      </button>
    </form>
  );
}

export function UnlockScreen({ mode, lockReason, needsSecretKey, usesSecretKey, onUnlocked }: Props) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [joining, setJoining] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [askKey, setAskKey] = useState(needsSecretKey);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => inputRef.current?.focus(), [joining]);

  const creating = mode === "create";
  const tooShort = creating && password.length > 0 && [...password].length < MIN_LENGTH;
  const mismatch = creating && confirm.length > 0 && confirm !== password;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !password) return;
    if (creating && ([...password].length < MIN_LENGTH || password !== confirm)) return;
    if (askKey && !secretKey) return;
    setBusy(true);
    setError(null);
    try {
      const status = creating
        ? await api.createVault(password)
        : await api.unlock(password, askKey ? secretKey : undefined);
      setPassword("");
      setConfirm("");
      setSecretKey("");
      onUnlocked(status, creating);
    } catch (err) {
      if (err instanceof ApiError && err.code === "secret_key_required") setAskKey(true);
      setError(err instanceof ApiError ? err.message : "Could not open the vault.");
      setPassword("");
      setBusy(false);
      inputRef.current?.focus();
    }
  }

  if (joining) {
    return (
      <main className="unlock">
        <JoinPanel onBack={() => setJoining(false)} onJoined={(s) => onUnlocked(s, false)} />
      </main>
    );
  }

  return (
    <main className="unlock">
      <form className="unlock-panel" onSubmit={submit} aria-busy={busy}>
        <Seal open={busy} />
        <h1 className="unlock-title">{creating ? "Create your vault" : "HavenKeys is locked"}</h1>
        <p className="unlock-lede">
          {creating
            ? "Choose a master password. It encrypts everything in the vault and cannot be recovered if you forget it."
            : askKey
              ? "Enter your master password and the Secret Key from your Emergency Kit. The Secret Key is remembered on this computer after you unlock."
              : ((lockReason && reasonText[lockReason]) ?? "Enter your master password to unlock.")}
        </p>

        <PasswordField
          label="Master password"
          value={password}
          onChange={setPassword}
          disabled={busy}
          invalid={!!error || tooShort}
          inputRef={inputRef}
        />
        {tooShort && <p className="unlock-hint">Use at least {MIN_LENGTH} characters. A few unrelated words work well.</p>}

        {creating && (
          <PasswordField
            label="Confirm master password"
            value={confirm}
            onChange={setConfirm}
            disabled={busy}
            invalid={mismatch}
          />
        )}
        {mismatch && <p className="unlock-hint">The passwords don’t match.</p>}

        {!creating && askKey && (
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
          {busy ? (creating ? "Creating vault…" : "Unlocking…") : creating ? "Create vault" : "Unlock"}
        </button>

        {!creating && !askKey && usesSecretKey && (
          <button className="btn btn-ghost unlock-submit" type="button" onClick={() => setAskKey(true)}>
            Enter Secret Key from Emergency Kit
          </button>
        )}

        {creating && (
          <>
            <p className="unlock-footnote">
              Your vault stays on this computer. A Secret Key is created with it; you’ll save it in an Emergency Kit
              next.
            </p>
            <button className="btn btn-ghost unlock-submit" type="button" onClick={() => setJoining(true)}>
              Already use HavenKeys? Set up from a sync folder
            </button>
          </>
        )}
      </form>
    </main>
  );
}

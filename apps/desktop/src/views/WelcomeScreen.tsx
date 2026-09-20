import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { api, ApiError } from "../lib/api";
import type { VaultStatus } from "../lib/types";
import { Seal } from "../components/Seal";

/**
 * First run on a computer that has no vault.
 *
 * Two ways in, and they are genuinely different acts: creating the account's
 * vault with an invite, or joining an account that already has one. Keeping
 * them on separate panels means neither form asks for a field the other path
 * needs, which is what made the old combined screens confusing.
 */

interface Props {
  /** Activation succeeded: the caller shows the Emergency Kit before the vault. */
  onActivated: (status: VaultStatus) => void;
  /** Sign-in succeeded: this device already has a kit somewhere else. */
  onSignedIn: (status: VaultStatus) => void;
}

type Path = "invite" | "signin";

function Field(props: {
  label: string;
  hint?: ReactNode;
  value: string;
  onChange: (value: string) => void;
  type?: "text" | "password" | "email";
  placeholder?: string;
  disabled?: boolean;
  mono?: boolean;
  inputRef?: React.Ref<HTMLInputElement>;
  autoComplete?: string;
}) {
  return (
    <label className="field">
      <span className="field-label">{props.label}</span>
      <input
        ref={props.inputRef}
        className={props.mono ? "field-input field-mono" : "field-input"}
        type={props.type ?? "text"}
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        placeholder={props.placeholder}
        disabled={props.disabled}
        autoComplete={props.autoComplete ?? "off"}
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
      />
      {props.hint && <span className="field-hint">{props.hint}</span>}
    </label>
  );
}

function InvitePanel({ onActivated }: { onActivated: (s: VaultStatus) => void }) {
  const [invite, setInvite] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => first.current?.focus(), []);

  const mismatch = confirm.length > 0 && confirm !== password;
  const tooShort = password.length > 0 && password.length < 10;
  const ready = invite.trim().length > 0 && password.length >= 10 && confirm === password;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !ready) return;
    setBusy(true);
    setError(null);
    try {
      onActivated(await api.activate(invite, password));
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not set up this vault.");
    } finally {
      setBusy(false);
      // Nothing keeps the typed password: React drops it with the state
      // below, and the core never echoes it back.
      setPassword("");
      setConfirm("");
    }
  }

  return (
    <form className="welcome-form" onSubmit={submit} noValidate>
      <label className="field">
        <span className="field-label">Invite</span>
        <textarea
          className="field-input field-mono field-area"
          value={invite}
          onChange={(e) => setInvite(e.target.value)}
          placeholder="HKINV1-…"
          rows={3}
          disabled={busy}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          ref={first as unknown as React.Ref<HTMLTextAreaElement>}
        />
        <span className="field-hint">
          One line, from whoever runs your HavenKeys server. It works once and expires after seven days.
        </span>
      </label>

      <Field
        label="Master password"
        type="password"
        value={password}
        onChange={setPassword}
        disabled={busy}
        hint="At least 10 characters. Nobody can reset it for you — not the server, not us."
      />
      <Field
        label="Repeat master password"
        type="password"
        value={confirm}
        onChange={setConfirm}
        disabled={busy}
      />

      {tooShort && <p className="form-error">Use at least 10 characters.</p>}
      {mismatch && <p className="form-error">The two passwords don’t match.</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <button className="btn btn-brass btn-lg" type="submit" disabled={busy || !ready}>
        {busy ? "Setting up…" : "Create my vault"}
      </button>
      <p className="welcome-note">
        Your master password and Secret Key never leave this computer. The server stores only encrypted data.
      </p>
    </form>
  );
}

function SignInPanel({ onSignedIn }: { onSignedIn: (s: VaultStatus) => void }) {
  const [server, setServer] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => first.current?.focus(), []);

  // The Secret Key can be left out when this computer already holds one:
  // a setup interrupted after the account was created finishes here.
  const ready = server.trim() && email.trim() && password;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !ready) return;
    setBusy(true);
    setError(null);
    try {
      onSignedIn(await api.signIn(server, email, password, secretKey));
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not sign in.");
    } finally {
      setBusy(false);
      setPassword("");
      setSecretKey("");
    }
  }

  return (
    <form className="welcome-form" onSubmit={submit} noValidate>
      <Field
        label="Server"
        value={server}
        onChange={setServer}
        placeholder="https://vault.example.com"
        disabled={busy}
        inputRef={first}
        hint="From your Emergency Kit."
      />
      <Field
        label="Email"
        type="email"
        value={email}
        onChange={setEmail}
        placeholder="you@example.com"
        disabled={busy}
      />
      <Field label="Master password" type="password" value={password} onChange={setPassword} disabled={busy} />
      <Field
        label="Secret Key"
        value={secretKey}
        onChange={setSecretKey}
        placeholder="H1-XXXXXX-XXXXXX-XXXXXX-XXXXXX"
        disabled={busy}
        mono
        hint="The long code on your Emergency Kit. Leave it empty if this computer already has it."
      />

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <button className="btn btn-brass btn-lg" type="submit" disabled={busy || !ready}>
        {busy ? "Signing in…" : "Sign in"}
      </button>
      <p className="welcome-note">
        Signing in downloads your vault and decrypts it here. The server never sees your password or your Secret Key.
      </p>
    </form>
  );
}

export function WelcomeScreen({ onActivated, onSignedIn }: Props) {
  const [path, setPath] = useState<Path>("invite");

  return (
    <main className="welcome">
      <div className="welcome-card">
        <header className="welcome-head">
          <Seal />
          <h1>HavenKeys</h1>
          <p className="welcome-sub">
            {path === "invite"
              ? "Set up this computer with your invite."
              : "Add this computer to an account you already have."}
          </p>
        </header>

        <div className="segmented" role="tablist" aria-label="How to set up">
          <button
            type="button"
            role="tab"
            aria-selected={path === "invite"}
            className={path === "invite" ? "segmented-item is-active" : "segmented-item"}
            onClick={() => setPath("invite")}
          >
            I have an invite
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={path === "signin"}
            className={path === "signin" ? "segmented-item is-active" : "segmented-item"}
            onClick={() => setPath("signin")}
          >
            I already have an account
          </button>
        </div>

        {path === "invite" ? <InvitePanel onActivated={onActivated} /> : <SignInPanel onSignedIn={onSignedIn} />}
      </div>
    </main>
  );
}

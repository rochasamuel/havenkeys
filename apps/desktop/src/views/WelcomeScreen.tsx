import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { api } from "../lib/api";
import type { InvitePreview, PasswordStrength, VaultStatus } from "../lib/types";
import { Guilloche } from "../components/Guilloche";
import { Seal } from "../components/Seal";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { PhoneSignInPanel } from "./PhoneSignInPanel";

/**
 * First run on a computer that has no vault.
 *
 * Two ways in, and they are genuinely different acts: creating the account's
 * vault with an invite, or joining an account that already has one. Keeping
 * them on separate panels means neither form asks for a field the other path
 * needs, which is what made the old combined screens confusing.
 */

interface Props {
  /** Activation succeeded: the caller shows the Recovery Sheet before the vault. */
  onActivated: (status: VaultStatus) => void;
  /** Sign-in succeeded: this device already has a kit somewhere else. */
  onSignedIn: (status: VaultStatus) => void;
  /** Signed in by the phone: the caller says which account it joined. */
  onPaired: (status: VaultStatus) => void;
}

type Path = "invite" | "signin" | "phone";

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
  /** Rendered between the input and the hint (the strength gauge). */
  after?: ReactNode;
}) {
  return (
    <label className="wf">
      <span className="wf-label">{props.label}</span>
      <input
        ref={props.inputRef}
        className={props.mono ? "wf-input mono" : "wf-input"}
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
      {props.after}
      {props.hint && <span className="wf-hint">{props.hint}</span>}
    </label>
  );
}

type StrengthLevel = "weak" | "fair" | "strong" | "excellent";

function strengthLevelOf(score: PasswordStrength["score"]): StrengthLevel {
  return score <= 1 ? "weak" : score === 2 ? "fair" : score === 3 ? "strong" : "excellent";
}

/**
 * The master password gauge: the generator's strength bar, driven by the
 * core's zxcvbn score instead of generator entropy. Rendered as soon as the
 * user types, so the bar is in place before the first score lands.
 */
function StrengthMeter({ strength }: { strength: PasswordStrength | null }) {
  const { t } = useI18n();
  const level = strength && strengthLevelOf(strength.score);
  const fill = strength ? Math.min(1, Math.max(0.06, strength.guessesLog10 / 12)) : 0;
  return (
    <div className={level ? `strength wf-strength strength-${level}` : "strength wf-strength"} aria-live="polite">
      <div className="strength-bar">
        <span style={{ transform: `scaleX(${fill})` }} />
      </div>
      {level && (
        <span>
          <strong>{t.generator.strength[level]}</strong> · {t.welcome.strengthNote[level]}
        </span>
      )}
    </div>
  );
}

function InvitePanel({ onActivated }: { onActivated: (s: VaultStatus) => void }) {
  const { t } = useI18n();
  const [invite, setInvite] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<InvitePreview | null>(null);
  const [strength, setStrength] = useState<PasswordStrength | null>(null);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => first.current?.focus(), []);

  // The code box grows with what is pasted (a code wraps onto several
  // lines) instead of scrolling inside a fixed height; see .wf-area.
  useEffect(() => {
    const area = first.current as unknown as HTMLTextAreaElement | null;
    if (!area) return;
    area.style.height = "auto";
    area.style.height = `${area.scrollHeight}px`;
  }, [invite]);

  // Score the draft after each pause in typing. The core scores and drops
  // it; the account email (from the code's preview) counts as an easy guess.
  useEffect(() => {
    if (password.length === 0) {
      setStrength(null);
      return;
    }
    let live = true;
    const inputs = preview ? [preview.email] : [];
    const timer = window.setTimeout(() => {
      api.estimateMasterPassword(password, inputs).then(
        (s) => live && setStrength(s),
        () => live && setStrength(null),
      );
    }, 150);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [password, preview]);

  // Say who the code is for before the user commits. Only a setup code is
  // sent to Rust; a failure (a typo, half a paste) just clears the line.
  useEffect(() => {
    const code = invite.trim();
    if (!code.startsWith("HKINV1-")) {
      setPreview(null);
      return;
    }
    let live = true;
    const timer = window.setTimeout(() => {
      api.previewInvite(code).then(
        (p) => live && setPreview(p),
        () => live && setPreview(null),
      );
    }, 300);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [invite]);

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
      setError(errorMessage(err, t, t.welcome.createFailed));
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
      <label className="wf">
        <span className="wf-label">{t.welcome.invite}</span>
        <textarea
          className="wf-input mono wf-area"
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
        <span className="wf-hint">{t.welcome.inviteHint}</span>
        {preview && <p className="wf-hint">{t.welcome.creatingFor(preview.email, preview.serverUrl)}</p>}
      </label>

      <Field
        label={t.common.masterPassword}
        type="password"
        value={password}
        onChange={setPassword}
        disabled={busy}
        after={password.length > 0 ? <StrengthMeter strength={strength} /> : null}
        hint={t.welcome.passwordHint}
      />
      <Field
        label={t.welcome.repeatPassword}
        type="password"
        value={confirm}
        onChange={setConfirm}
        disabled={busy}
      />

      {tooShort && <p className="form-error">{t.welcome.tooShort}</p>}
      {mismatch && <p className="form-error">{t.welcome.mismatch}</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <button className="btn btn-primary btn-lg" type="submit" disabled={busy || !ready}>
        {busy ? t.welcome.creating : t.welcome.create}
      </button>
      <p className="welcome-note">{t.welcome.createNote}</p>
    </form>
  );
}

function SignInPanel({ onSignedIn }: { onSignedIn: (s: VaultStatus) => void }) {
  const { t } = useI18n();
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
      setError(errorMessage(err, t, t.welcome.signInFailed));
    } finally {
      setBusy(false);
      setPassword("");
      setSecretKey("");
    }
  }

  return (
    <form className="welcome-form" onSubmit={submit} noValidate>
      <Field
        label={t.common.server}
        value={server}
        onChange={setServer}
        placeholder="https://vault.example.com"
        disabled={busy}
        inputRef={first}
        hint={t.welcome.serverHint}
      />
      <Field
        label={t.common.email}
        type="email"
        value={email}
        onChange={setEmail}
        placeholder={t.welcome.emailPlaceholder}
        disabled={busy}
      />
      <Field label={t.common.masterPassword} type="password" value={password} onChange={setPassword} disabled={busy} />
      <Field
        label={t.common.secretKey}
        value={secretKey}
        onChange={setSecretKey}
        placeholder="H1-XXXXXX-XXXXXX-XXXXXX-XXXXXX"
        disabled={busy}
        mono
        hint={t.welcome.secretKeyHint}
      />

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <button className="btn btn-primary btn-lg" type="submit" disabled={busy || !ready}>
        {busy ? t.welcome.signingIn : t.welcome.signIn}
      </button>
      <p className="welcome-note">{t.welcome.signInNote}</p>
    </form>
  );
}

export function WelcomeScreen({ onActivated, onSignedIn, onPaired }: Props) {
  const { t } = useI18n();
  const [path, setPath] = useState<Path>("invite");

  return (
    <main className="welcome" data-tauri-drag-region>
      <Guilloche className="unlock-rosette" />
      <div className="welcome-card">
        <header className="welcome-head">
          <Seal size={60} />
          <h1>{t.welcome.title}</h1>
          <p className="welcome-sub">
            {path === "invite" ? t.welcome.subInvite : path === "phone" ? t.welcome.subPhone : t.welcome.subSignIn}
          </p>
        </header>

        <button className="btn btn-primary btn-lg" type="button" onClick={() => void api.openSignup()}>
          {t.welcome.createAccount}
        </button>
        <p className="welcome-note">{t.welcome.createAccountNote}</p>

        <div className="segmented" role="tablist" aria-label={t.welcome.howToSetUp}>
          <button
            type="button"
            role="tab"
            aria-selected={path === "invite"}
            className={path === "invite" ? "segmented-item is-active" : "segmented-item"}
            onClick={() => setPath("invite")}
          >
            {t.welcome.tabInvite}
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={path === "signin"}
            className={path === "signin" ? "segmented-item is-active" : "segmented-item"}
            onClick={() => setPath("signin")}
          >
            {t.welcome.tabSignIn}
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={path === "phone"}
            className={path === "phone" ? "segmented-item is-active" : "segmented-item"}
            onClick={() => setPath("phone")}
          >
            {t.welcome.tabPhone}
          </button>
        </div>

        {path === "invite" && <InvitePanel onActivated={onActivated} />}
        {path === "signin" && <SignInPanel onSignedIn={onSignedIn} />}
        {path === "phone" && <PhoneSignInPanel onSignedIn={onPaired} onUseKit={() => setPath("signin")} />}
      </div>
    </main>
  );
}

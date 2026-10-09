import { useEffect, useReducer, useRef, useState, type Dispatch, type FormEvent } from "react";
import { AndroidDownload } from "../components/AndroidDownload";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";
import type { Messages } from "../i18n/en";
import type { Locale } from "../i18n/locale";
import { startSignup, verifySignup } from "../lib/api";
import { TERMS_VERSION } from "../lib/legal";
import { detectOs, type Os } from "../lib/os";
import { fetchLatestRelease, pickAsset, RELEASES_PAGE_URL, type LatestRelease, type Platform } from "../lib/releases";
import { canResend, initialSignup, RESEND_AFTER_MS, signupReducer, type SignupAction, type SignupState } from "../lib/signup";

/*
 * The invite is React state and nothing else. It is never written to
 * storage, the URL, the title or analytics, and it goes when this component
 * unmounts (a language switch remounts the page: App keys it by locale).
 */
export function Signup() {
  const { t, locale } = useI18n();
  const [state, dispatch] = useReducer(signupReducer, undefined, initialSignup);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (state.step !== "code") return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [state.step]);

  useEffect(() => {
    if (state.step === "email" && state.busy) {
      let cancelled = false;
      startSignup({ email: state.email, locale, acceptedTerms: TERMS_VERSION }).then((r) => {
        if (cancelled) return;
        dispatch(r.ok ? { type: "sent", at: Date.now() } : { type: "failed", error: r.failure.kind });
      });
      return () => {
        cancelled = true;
      };
    }
    if (state.step === "code" && state.busy && state.code === "") {
      let cancelled = false;
      startSignup({ email: state.email, locale, acceptedTerms: TERMS_VERSION }).then((r) => {
        if (cancelled) return;
        dispatch(r.ok ? { type: "sent", at: Date.now() } : { type: "failed", error: r.failure.kind });
      });
      return () => {
        cancelled = true;
      };
    }
    if (state.step === "code" && state.busy) {
      let cancelled = false;
      verifySignup({ email: state.email, code: state.code }).then((r) => {
        if (cancelled) return;
        dispatch(r.ok ? { type: "verified", invite: r.invite } : { type: "failed", error: r.failure.kind });
      });
      return () => {
        cancelled = true;
      };
    }
    return undefined;
  }, [state, locale]);

  return <SignupView state={state} now={now} dispatch={dispatch} t={t} locale={locale} />;
}

export function SignupView({
  state,
  now,
  dispatch,
  t,
  locale,
}: {
  state: SignupState;
  now: number;
  dispatch: Dispatch<SignupAction>;
  t: Messages;
  locale: Locale;
}) {
  const s = t.signup;
  const path = (p: string) => (locale === "pt-BR" ? `/pt-br${p}` : p);
  return (
    <section className="signup">
      <div className="signup__card">
        {state.step === "email" && (
          <form
            onSubmit={(e: FormEvent) => {
              e.preventDefault();
              dispatch({ type: "submit_email" });
            }}
            noValidate
          >
            <h1>{s.title}</h1>
            <p className="signup__lede">{s.lede}</p>
            <p className="signup__note">{t.common.betaNotice}</p>
            <label className="field">
              <span className="field__label">{s.emailLabel}</span>
              <input
                className="field__input"
                type="email"
                autoComplete="email"
                inputMode="email"
                value={state.email}
                disabled={state.busy}
                onChange={(e) => dispatch({ type: "email", value: e.target.value })}
              />
              <span className="field__hint">{s.emailHint}</span>
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={state.accepted}
                disabled={state.busy}
                onChange={(e) => dispatch({ type: "accept", value: e.target.checked })}
              />
              <span>{s.terms(path("/terms"), path("/privacy"))}</span>
            </label>
            {state.error && (
              <p className="field__error" role="alert">
                {s.errors[state.error]}
              </p>
            )}
            <button className="btn btn--primary btn--lg" type="submit" disabled={state.busy}>
              {state.busy ? s.sending : s.create}
            </button>
          </form>
        )}

        {state.step === "code" && (
          <form
            onSubmit={(e: FormEvent) => {
              e.preventDefault();
              dispatch({ type: "submit_code" });
            }}
            noValidate
          >
            <h1>{s.codeTitle}</h1>
            <p className="signup__lede">{s.codeLede(state.email)}</p>
            <label className="field">
              <span className="field__label">{s.codeLabel}</span>
              <input
                className="field__input code-input"
                type="text"
                inputMode="numeric"
                autoComplete="one-time-code"
                pattern="[0-9]*"
                maxLength={6}
                value={state.code}
                disabled={state.busy}
                onChange={(e) => dispatch({ type: "code", value: e.target.value })}
              />
            </label>
            {state.error && (
              <p className="field__error" role="alert">
                {s.errors[state.error]}
              </p>
            )}
            <div className="signup__actions">
              <button className="btn btn--primary btn--lg" type="submit" disabled={state.busy}>
                {state.busy ? s.verifying : s.verify}
              </button>
              <button
                className="btn btn--ghost"
                type="button"
                disabled={!canResend(state, now)}
                onClick={() => dispatch({ type: "resend" })}
              >
                {canResend(state, now)
                  ? s.resend
                  : s.resendIn.replace("{s}", String(Math.max(0, Math.ceil((state.sentAt + RESEND_AFTER_MS - now) / 1000))))}
              </button>
              <button className="btn btn--ghost" type="button" onClick={() => dispatch({ type: "reset" })}>
                {s.changeEmail}
              </button>
            </div>
          </form>
        )}

        {state.step === "done" && <Done invite={state.invite} t={t} />}
      </div>
    </section>
  );
}

function Done({ invite, t }: { invite: string; t: Messages }) {
  const s = t.signup;
  const [copied, setCopied] = useState(false);
  const field = useRef<HTMLInputElement>(null);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(invite);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      field.current?.select();
    }
  };
  return (
    <div>
      <h1>{s.doneTitle}</h1>
      <p className="signup__lede">{s.doneLede}</p>
      <div className="invite">
        <input
          ref={field}
          className="field__input invite__field"
          readOnly
          value={invite}
          aria-label={s.inviteAria}
          onFocus={(e) => e.target.select()}
        />
        <button className="btn btn--primary" type="button" onClick={copy}>
          <Icon name="copy" size={16} />
          {copied ? s.copied : s.copy}
        </button>
      </div>
      <p className="signup__note">{s.alsoEmailed}</p>
      <h2>{s.downloadsTitle}</h2>
      <Downloads t={t} />
      <p className="signup__note">{t.common.installerWarning}</p>
      <p className="signup__note">{t.common.betaNotice}</p>
    </div>
  );
}

const DESKTOP: Array<{ id: Platform; os: Os }> = [
  { id: "windows", os: "windows" },
  { id: "macos", os: "macos" },
  { id: "linux-appimage", os: "linux" },
];

function Downloads({ t }: { t: Messages }) {
  const [release, setRelease] = useState<LatestRelease | null>(null);
  const [os] = useState(() => (typeof navigator === "undefined" ? null : detectOs(navigator.userAgent)));
  useEffect(() => {
    let cancelled = false;
    fetchLatestRelease().then((r) => {
      if (!cancelled) setRelease(r);
    });
    return () => {
      cancelled = true;
    };
  }, []);
  return (
    <div className="signup__downloads">
      {DESKTOP.map((p) => {
        const asset = release ? pickAsset(release.assets, p.id) : null;
        const yours = p.os === os;
        return (
          <a
            key={p.id}
            className={`btn ${yours ? "btn--primary" : "btn--ghost"}`}
            href={asset?.browser_download_url ?? RELEASES_PAGE_URL}
            target="_blank"
            rel="noreferrer"
          >
            <Icon name="download" size={15} />
            {t.download.platforms[p.id].label}
          </a>
        );
      })}
      {os === "android" && <AndroidDownload yours />}
      <a className="text-link" href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
        {t.signup.otherDownloads}
      </a>
    </div>
  );
}

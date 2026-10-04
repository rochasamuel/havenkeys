import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import { nextStep, secondsLeft } from "../lib/pairing";
import type { PairingCode, VaultStatus } from "../lib/types";
import { QrCode } from "../components/EmergencyKit";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

const POLL_MS = 2000;

type Phase =
  | { kind: "server" }
  | { kind: "code"; code: PairingCode }
  | { kind: "expired" }
  | { kind: "denied" };

/** The server, then the code, then the outcome. Keys never reach React. */
export function PhoneSignInPanel({
  onSignedIn,
  onUseKit,
}: {
  onSignedIn: (s: VaultStatus) => void;
  onUseKit: () => void;
}) {
  const { t } = useI18n();
  const [server, setServer] = useState("");
  const [phase, setPhase] = useState<Phase>({ kind: "server" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => first.current?.focus(), []);
  // Leaving the panel ends the pairing on this side.
  useEffect(() => () => void api.pairingCancel().catch(() => {}), []);

  useEffect(() => {
    if (phase.kind !== "code") return;
    const tick = setInterval(() => setNow(Date.now()), 1000);
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const answer = await api.pairingPoll();
        if (stopped) return;
        const step = nextStep(answer.state);
        if (step === "done" && answer.state === "approved") onSignedIn(answer.status);
        else if (step === "denied") setPhase({ kind: "denied" });
        else if (step === "expired") setPhase({ kind: "expired" });
        else timer = setTimeout(poll, POLL_MS);
      } catch (err) {
        if (stopped) return;
        setError(errorMessage(err, t, t.welcome.signInFailed));
        timer = setTimeout(poll, POLL_MS);
      }
    };
    timer = setTimeout(poll, POLL_MS);
    return () => {
      stopped = true;
      clearInterval(tick);
      clearTimeout(timer);
    };
    // `t` is left out: a language change must not restart the pairing.
  }, [phase, onSignedIn]);

  const left = phase.kind === "code" ? secondsLeft(phase.code.expiresAt, now) : 0;
  useEffect(() => {
    if (phase.kind === "code" && left === 0) setPhase({ kind: "expired" });
  }, [phase, left]);

  async function start(e?: FormEvent) {
    e?.preventDefault();
    if (busy || !server.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const code = await api.pairingStart(server);
      setNow(Date.now());
      setPhase({ kind: "code", code });
    } catch (err) {
      setError(errorMessage(err, t, t.welcome.signInFailed));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="welcome-form">
      {phase.kind === "server" && (
        <form className="welcome-form" onSubmit={start} noValidate>
          <label className="wf">
            <span className="wf-label">{t.welcome.phoneServer}</span>
            <input
              ref={first}
              className="wf-input"
              value={server}
              onChange={(e) => setServer(e.target.value)}
              placeholder="https://vault.example.com"
              disabled={busy}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
            />
            <span className="wf-hint">{t.welcome.serverHint}</span>
          </label>
          <button className="btn btn-primary btn-lg" type="submit" disabled={busy || !server.trim()}>
            {busy ? t.welcome.phoneStarting : t.welcome.phoneShowCode}
          </button>
        </form>
      )}

      {phase.kind === "code" && (
        <div className="pair-code">
          <QrCode
            size={phase.code.qrSize}
            modules={phase.code.qrModules}
            label={t.welcome.phoneQrLabel}
            className="pair-qr"
          />
          <p className="pair-help">{t.welcome.phoneScan}</p>
          <p className="pair-left" aria-live="polite">
            {t.welcome.phoneExpiresIn(left)}
          </p>
        </div>
      )}

      {(phase.kind === "expired" || phase.kind === "denied") && (
        <div className="pair-code">
          <p role="alert">{phase.kind === "expired" ? t.welcome.phoneExpired : t.welcome.phoneDenied}</p>
          <button className="btn btn-primary" type="button" onClick={() => void start()} disabled={busy}>
            {t.welcome.phoneNewCode}
          </button>
        </div>
      )}

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <button className="btn btn-quiet" type="button" onClick={onUseKit}>
        {t.welcome.phoneUseKit}
      </button>
    </div>
  );
}

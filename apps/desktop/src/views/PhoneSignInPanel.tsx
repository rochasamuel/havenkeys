import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import { isTerminalPollError, nextStep, PAIRING_SECONDS, secondsLeft } from "../lib/pairing";
import type { PairingCode, VaultStatus } from "../lib/types";
import { QrCode } from "../components/EmergencyKit";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

const POLL_MS = 2000;

type Phase =
  | { kind: "server" }
  /** `deadline`: Unix ms on this computer's clock, never the server's. */
  | { kind: "code"; code: PairingCode; deadline: number }
  | { kind: "expired" }
  | { kind: "denied" }
  /** Ended by Rust (pairing_failed, pairing_gone); `message` is the first one. */
  | { kind: "failed"; message: string };

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
  // The parent passes a new function every render; the poll must not restart for it.
  const signedIn = useRef(onSignedIn);
  signedIn.current = onSignedIn;

  useEffect(() => first.current?.focus(), []);
  // Leaving the panel ends the pairing on this side.
  useEffect(() => () => void api.pairingCancel().catch(() => {}), []);

  useEffect(() => {
    if (phase.kind !== "code") return;
    const { deadline } = phase;
    const tick = setInterval(() => setNow(Date.now()), 1000);
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    // The next poll, at most at the deadline: the last one runs at 0, so an
    // approval made in the final seconds still arrives. False once that last
    // poll has answered.
    const next = () => {
      const wait = deadline - Date.now();
      if (wait <= 0) return false;
      timer = setTimeout(poll, Math.min(POLL_MS, wait));
      return true;
    };
    const poll = async () => {
      try {
        const answer = await api.pairingPoll();
        // Polling is consume-once: an approval means Rust has already signed this
        // computer in, so it must reach the app even after the effect was torn down.
        if (answer.state === "approved") {
          signedIn.current(answer.status);
          return;
        }
        if (stopped) return;
        setError(null);
        const step = nextStep(answer.state);
        if (step === "denied") setPhase({ kind: "denied" });
        else if (step === "expired" || !next()) setPhase({ kind: "expired" });
      } catch (err) {
        if (stopped) return;
        const message = errorMessage(err, t, t.welcome.signInFailed);
        // Rust has ended the pairing: polling again could only replace this
        // message with a less accurate one.
        if (err instanceof ApiError && isTerminalPollError(err.code)) {
          setError(null);
          setPhase({ kind: "failed", message });
          return;
        }
        // Offline or no answer: keep trying until the deadline.
        setError(message);
        if (!next()) setPhase({ kind: "expired" });
      }
    };
    next();
    return () => {
      stopped = true;
      clearInterval(tick);
      clearTimeout(timer);
    };
    // `t` is left out: a language change must not restart the pairing.
  }, [phase]);

  const left = phase.kind === "code" ? secondsLeft(phase.deadline, now) : 0;

  async function start(e?: FormEvent) {
    e?.preventDefault();
    if (busy || !server.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const code = await api.pairingStart(server);
      const at = Date.now();
      setNow(at);
      setPhase({ kind: "code", code, deadline: at + PAIRING_SECONDS * 1000 });
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

      {(phase.kind === "expired" || phase.kind === "denied" || phase.kind === "failed") && (
        <div className="pair-code">
          <p role="alert">
            {phase.kind === "expired"
              ? t.welcome.phoneExpired
              : phase.kind === "denied"
                ? t.welcome.phoneDenied
                : phase.message}
          </p>
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

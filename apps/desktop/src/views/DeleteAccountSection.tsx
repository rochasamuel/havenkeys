import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

type Step = "closed" | "explain" | "confirm";

const normalized = (email: string) => email.trim().toLowerCase();

/**
 * Settings → "Delete account and all data" (spec
 * 2026-10-05-account-deletion §6). Rust re-checks everything shown here:
 * unlocked, online, the typed email and the master password.
 */
export function DeleteAccountSection({ online }: { online: boolean }) {
  const { t } = useI18n();
  const [step, setStep] = useState<Step>("closed");
  const [email, setEmail] = useState("");
  const [typed, setTyped] = useState("");
  const [master, setMaster] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.accountStatus().then(
      (a) => setEmail(a?.email ?? ""),
      () => setEmail(""),
    );
  }, []);
  // The password never outlives the section.
  useEffect(() => () => setMaster(""), []);

  const matches = email !== "" && normalized(typed) === normalized(email);

  function close() {
    setMaster("");
    setTyped("");
    setError(null);
    setStep("closed");
  }

  async function remove() {
    setBusy(true);
    setError(null);
    try {
      await api.deleteAccount(typed, master);
      // App.tsx handles vault://account-deleted and returns to first run.
    } catch (e) {
      setError(errorMessage(e, t, t.deleteAccount.failed));
    } finally {
      setMaster("");
      setBusy(false);
    }
  }

  return (
    <div className="settings-block">
      <h3 className="group-title">{t.deleteAccount.title}</h3>
      <div className="group danger-zone">
        <p className="group-note group-note-top">{online ? t.deleteAccount.explain : t.deleteAccount.offline}</p>
        {step === "closed" && (
          <div className="group-actions">
            <button className="btn btn-danger" disabled={!online} onClick={() => setStep("explain")}>
              {t.deleteAccount.title}
            </button>
          </div>
        )}
        {step === "explain" && (
          <>
            <p className="group-note">{t.deleteAccount.keptNote}</p>
            <div className="group-actions">
              <button
                className="btn btn-primary"
                onClick={() => document.getElementById("export")?.scrollIntoView({ behavior: "smooth" })}
              >
                {t.deleteAccount.backupFirst}
              </button>
              <button className="btn" onClick={() => setStep("confirm")}>
                {t.deleteAccount.continueWithout}
              </button>
              <button className="btn" onClick={close}>
                {t.common.cancel}
              </button>
            </div>
          </>
        )}
        {step === "confirm" && (
          <>
            <label className="row row-input">
              <span>{t.deleteAccount.typeToConfirm(email)}</span>
              <input
                name="confirm-email"
                value={typed}
                autoComplete="off"
                spellCheck={false}
                disabled={busy}
                onChange={(e) => setTyped(e.target.value)}
              />
            </label>
            <label className="row row-input">
              <span>{t.deleteAccount.masterPassword}</span>
              <input
                type="password"
                name="master-password"
                autoComplete="current-password"
                value={master}
                disabled={busy}
                onChange={(e) => setMaster(e.target.value)}
              />
            </label>
            {error && (
              <p className="form-error" role="alert">
                {error}
              </p>
            )}
            <div className="group-actions">
              <button
                className="btn btn-danger"
                disabled={!matches || !master || busy || !online}
                onClick={remove}
              >
                {t.deleteAccount.confirm}
              </button>
              <button className="btn" disabled={busy} onClick={close}>
                {t.common.cancel}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { ExportFormat, ExportResult, ExportSummary } from "../lib/types";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import type { Messages } from "../i18n/en";

const FORMATS: ExportFormat[] = ["backup", "bitwardenJson", "csv"];
const EXTENSION: Record<ExportFormat, string> = { backup: "hkbackup", bitwardenJson: "json", csv: "csv" };
const MIN_BACKUP_PASSWORD = 10;

function leftOut(s: ExportSummary, t: Messages): string[] {
  const out: string[] = [];
  if (s.passkeysLeftOut) out.push(t.export.passkeysLeftOut(s.passkeysLeftOut));
  if (s.passwordHistoryLeftOut) out.push(t.export.historyLeftOut(s.passwordHistoryLeftOut));
  if (s.customFieldsLeftOut) out.push(t.export.fieldsLeftOut(s.customFieldsLeftOut));
  if (s.itemsLeftOut) out.push(t.export.itemsLeftOut(s.itemsLeftOut));
  if (s.unreadable) out.push(t.export.unreadable(s.unreadable));
  return out;
}

function exportedCount(s: ExportSummary, format: ExportFormat): number {
  const all = s.logins + s.secureNotes + s.cards + s.identities;
  return format === "csv" ? s.logins : all;
}

export function ExportSection() {
  const toast = useToast();
  const { t } = useI18n();
  const [format, setFormat] = useState<ExportFormat>("backup");
  const [summary, setSummary] = useState<ExportSummary | null>(null);
  const [summaryFailed, setSummaryFailed] = useState(false);
  const [understood, setUnderstood] = useState(false);
  const [master, setMaster] = useState("");
  const [backup, setBackup] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ format: ExportFormat; r: ExportResult } | null>(null);

  useEffect(() => {
    let live = true;
    setSummary(null);
    setSummaryFailed(false);
    setUnderstood(false);
    api.exportSummary(format).then(
      (s) => live && setSummary(s),
      () => live && setSummaryFailed(true),
    );
    return () => {
      live = false;
    };
  }, [format]);

  // Passwords never outlive the section.
  useEffect(
    () => () => {
      setMaster("");
      setBackup("");
      setAgain("");
    },
    [],
  );

  const isBackup = format === "backup";
  const mismatch = isBackup && again.length > 0 && backup !== again;
  // What the file will hold is shown before it is written (spec §4).
  const ready =
    summary !== null &&
    master.length > 0 &&
    (isBackup ? backup.length >= MIN_BACKUP_PASSWORD && backup === again : understood);
  const lines = summary ? leftOut(summary, t) : [];

  async function runExport() {
    setBusy(true);
    try {
      const r = await api.exportFile(format, master, isBackup ? backup : null);
      if (r) setResult({ format, r });
    } catch (e) {
      toast(errorMessage(e, t, t.export.failed), "error");
    } finally {
      setMaster("");
      setBackup("");
      setAgain("");
      setBusy(false);
    }
  }

  return (
    <div className="settings-block">
      <div className="group" role="radiogroup" aria-label={t.export.formatLabel}>
        {FORMATS.map((f) => (
          <label key={f} className="row choice-row">
            <input
              type="radio"
              name="export-format"
              value={f}
              checked={format === f}
              onChange={() => setFormat(f)}
              disabled={busy}
            />
            <span className="choice-text">
              <span className="choice-title">
                {t.export.formats[f]}
                {f === "backup" && <span className="pill">{t.export.recommended}</span>}
              </span>
              <span className="choice-desc">{t.export.formatHelp[f]}</span>
            </span>
            <span className="choice-ext">.{EXTENSION[f]}</span>
          </label>
        ))}
      </div>

      {summary && (
        <div className="group">
          <div className="row">
            <div className="row-main">
              <div className="row-label">{t.export.contains}</div>
              <div className="row-value">
                {t.export.includes(summary.logins, summary.secureNotes, summary.cards, summary.identities)}
              </div>
            </div>
          </div>
          {lines.length > 0 && (
            <ul className="group-note left-out">
              {lines.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ul>
          )}
        </div>
      )}
      {summaryFailed && (
        <p className="form-error" role="alert">
          {t.export.summaryFailed}
        </p>
      )}

      {isBackup ? (
        <>
          <div className="group">
            <label className="row row-input">
              <span className="row-label-inline">{t.export.backupPassword}</span>
              <input type="password" name="backup-password" autoComplete="new-password" value={backup} onChange={(e) => setBackup(e.target.value)} disabled={busy} />
            </label>
            <label className="row row-input">
              <span className="row-label-inline">{t.export.backupPasswordAgain}</span>
              <input
                type="password"
                name="backup-password-again"
                autoComplete="new-password"
                aria-invalid={mismatch || undefined}
                value={again}
                onChange={(e) => setAgain(e.target.value)}
                disabled={busy}
              />
            </label>
          </div>
          {mismatch ? (
            <p className="form-error" role="alert">
              {t.export.mismatch}
            </p>
          ) : (
            <p className="group-note">{t.export.backupHint}</p>
          )}
        </>
      ) : (
        <div className="group danger-zone">
          <div className="group-note warning-note" role="alert">
            <Icon name="alert" size={18} />
            <p>{t.export.plaintextWarning}</p>
          </div>
          <label className="row check-row">
            <input type="checkbox" name="understand" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} disabled={busy} />
            <span className="row-label-inline">{t.export.understand}</span>
          </label>
        </div>
      )}

      <div className="group">
        <label className="row row-input">
          <span className="row-label-inline">{t.export.masterPassword}</span>
          <input type="password" name="master-password" autoComplete="current-password" value={master} onChange={(e) => setMaster(e.target.value)} disabled={busy} />
        </label>
      </div>
      <div className="group-actions">
        <button className={isBackup ? "btn btn-primary" : "btn btn-danger"} onClick={() => void runExport()} disabled={busy || !ready}>
          {busy ? t.export.exporting : t.export.export}
        </button>
      </div>

      {result && (
        <div className="import-result" role="status">
          <p>
            <strong>{t.export.done(exportedCount(result.r.summary, result.format), result.r.fileName)}</strong>
          </p>
          {result.format !== "backup" && <p className="muted">{t.export.deleteReminder}</p>}
        </div>
      )}
    </div>
  );
}

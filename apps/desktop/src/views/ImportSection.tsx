import { useState } from "react";
import { api } from "../lib/api";
import type { ImportResult, ImportSource } from "../lib/types";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import type { Messages } from "../i18n/en";

type ImportChoice = ImportSource | "havenKeysBackup";

/** In the order the list shows them. */
const SOURCES: ImportChoice[] = [
  "onePassword",
  "bitwardenJson",
  "bitwardenCsv",
  "chrome",
  "firefox",
  "keePassXc",
  "lastPass",
  "havenKeysBackup",
];

const EXTENSION: Record<ImportChoice, string> = {
  onePassword: "1pux",
  bitwardenJson: "json",
  bitwardenCsv: "csv",
  chrome: "csv",
  firefox: "csv",
  keePassXc: "csv",
  lastPass: "csv",
  havenKeysBackup: "hkbackup",
};

/** Lines describing what was left out or changed; empty when nothing was. */
function caveats(r: ImportResult["report"], t: Messages, restored: boolean): string[] {
  const out: string[] = [];
  if (r.convertedToNotes) out.push(t.import.convertedToNotes(r.convertedToNotes));
  if (r.skippedDuplicates) out.push(t.import.skippedDuplicates(r.skippedDuplicates));
  if (r.skippedExisting) out.push(t.import.skippedExisting(r.skippedExisting));
  if (r.skippedArchived) out.push(t.import.skippedArchived(r.skippedArchived));
  if (r.attachmentsSkipped) out.push(t.import.attachmentsSkipped(r.attachmentsSkipped));
  if (r.passwordHistorySkipped) out.push(t.import.passwordHistorySkipped(r.passwordHistorySkipped));
  if (r.urlsMovedToNotes) out.push(t.import.urlsMovedToNotes(r.urlsMovedToNotes));
  if (r.fieldsToNotes) out.push(t.import.fieldsToNotes(r.fieldsToNotes));
  if (r.ssoUpgraded) out.push(t.import.ssoUpgraded(r.ssoUpgraded));
  if (r.passkeysSkipped) out.push(t.import.passkeysSkipped(r.passkeysSkipped));
  if (r.failed) out.push(restored ? t.import.restoreFailedItems(r.failed) : t.import.failedItems(r.failed));
  return out;
}

export function ImportSection({ onImported }: { onImported: () => void }) {
  const toast = useToast();
  const { t } = useI18n();
  const [source, setSource] = useState<ImportChoice>("onePassword");
  const [restorePassword, setRestorePassword] = useState("");
  const [restored, setRestored] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [deleted, setDeleted] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  async function runImport() {
    setBusy(true);
    try {
      const isRestore = source === "havenKeysBackup";
      const r = isRestore ? await api.restoreBackup(restorePassword) : await api.importFile(source);
      if (r) {
        setResult(r);
        setRestored(isRestore);
        setDeleted(false);
        setConfirmDelete(false);
        onImported();
      }
    } catch (e) {
      toast(errorMessage(e, t, t.import.failed), "error");
    } finally {
      setRestorePassword("");
      setBusy(false);
    }
  }

  async function deleteFile() {
    try {
      await api.deleteImportFile();
      setDeleted(true);
      setConfirmDelete(false);
      toast(t.import.fileDeleted);
    } catch (e) {
      toast(errorMessage(e, t, t.import.deleteFailed), "error");
    }
  }

  return (
    <div className="settings-block">
      <p className="group-note group-note-top">{t.import.note}</p>
      <div className="group" role="radiogroup" aria-label={t.import.sourceLabel}>
        {SOURCES.map((s) => (
          <label key={s} className="row choice-row">
            <input
              type="radio"
              name="import-source"
              value={s}
              checked={source === s}
              onChange={() => {
                setSource(s);
                // The backup password never outlives the backup source.
                if (s !== "havenKeysBackup") setRestorePassword("");
              }}
              disabled={busy}
            />
            <span className="choice-text">
              <span className="choice-title">{t.import.sources[s]}</span>
              {source === s && <span className="choice-desc">{t.import.howTo[s]}</span>}
            </span>
            <span className="choice-ext">.{EXTENSION[s]}</span>
          </label>
        ))}
      </div>
      {source === "havenKeysBackup" && (
        <div className="group">
          <label className="row row-input">
            <span className="row-label-inline">{t.import.backupPassword}</span>
            <input
              type="password"
              name="restore-password"
              autoComplete="off"
              value={restorePassword}
              onChange={(e) => setRestorePassword(e.target.value)}
              disabled={busy}
            />
          </label>
        </div>
      )}
      <div className="group-actions">
        <button
          className="btn btn-primary"
          onClick={() => void runImport()}
          disabled={busy || (source === "havenKeysBackup" && restorePassword.length === 0)}
        >
          {busy ? t.import.importing : t.import.choose(EXTENSION[source])}
        </button>
      </div>

      {result && (
        <div className="import-result" role="status">
          <p>
            <strong>{t.import.imported(result.report.imported)}</strong>
            {t.import.summary(
              result.fileName,
              result.report.logins,
              result.report.secureNotes,
              result.report.cards,
              result.report.identities,
            )}
          </p>
          {caveats(result.report, t, restored).length > 0 && (
            <ul>
              {caveats(result.report, t, restored).map((c) => (
                <li key={c}>{c}</li>
              ))}
            </ul>
          )}
          {!restored && (deleted ? (
            <p className="muted">{t.import.wasDeleted}</p>
          ) : confirmDelete ? (
            <div className="confirm">
              <span>{t.import.confirmDelete(result.fileName)}</span>
              <button className="btn btn-small btn-danger" onClick={() => void deleteFile()}>
                {t.import.deleteFile}
              </button>
              <button className="btn btn-small" onClick={() => setConfirmDelete(false)}>
                {t.import.keepFile}
              </button>
            </div>
          ) : (
            <div>
              <button className="btn btn-small" onClick={() => setConfirmDelete(true)}>
                {t.import.deleteExport}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

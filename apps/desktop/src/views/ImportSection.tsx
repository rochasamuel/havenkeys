import { useState } from "react";
import { api } from "../lib/api";
import type { ImportResult } from "../lib/types";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import type { Messages } from "../i18n/en";

/** Lines describing what was left out or changed; empty when nothing was. */
function caveats(r: ImportResult["report"], t: Messages): string[] {
  const out: string[] = [];
  if (r.convertedToNotes) out.push(t.import.convertedToNotes(r.convertedToNotes));
  if (r.skippedDuplicates) out.push(t.import.skippedDuplicates(r.skippedDuplicates));
  if (r.skippedArchived) out.push(t.import.skippedArchived(r.skippedArchived));
  if (r.attachmentsSkipped) out.push(t.import.attachmentsSkipped(r.attachmentsSkipped));
  if (r.passwordHistorySkipped) out.push(t.import.passwordHistorySkipped(r.passwordHistorySkipped));
  if (r.urlsMovedToNotes) out.push(t.import.urlsMovedToNotes(r.urlsMovedToNotes));
  if (r.failed) out.push(t.import.failedItems(r.failed));
  return out;
}

export function ImportSection({ onImported }: { onImported: () => void }) {
  const toast = useToast();
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [deleted, setDeleted] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  async function runImport() {
    setBusy(true);
    try {
      const r = await api.import1pux();
      if (r) {
        setResult(r);
        setDeleted(false);
        setConfirmDelete(false);
        onImported();
      }
    } catch (e) {
      toast(errorMessage(e, t, t.import.failed), "error");
    } finally {
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
      <h3 className="group-title">{t.import.title}</h3>
      <p className="group-note group-note-top">{t.import.note}</p>
      <div className="group-actions">
        <button className="btn" onClick={() => void runImport()} disabled={busy}>
          {busy ? t.import.importing : t.import.choose}
        </button>
      </div>

      {result && (
        <div className="import-result" role="status">
          <p>
            <strong>{t.import.imported(result.report.imported)}</strong>
            {t.import.summary(result.fileName, result.report.logins, result.report.secureNotes)}
          </p>
          {caveats(result.report, t).length > 0 && (
            <ul>
              {caveats(result.report, t).map((c) => (
                <li key={c}>{c}</li>
              ))}
            </ul>
          )}
          {deleted ? (
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
          )}
        </div>
      )}
    </div>
  );
}

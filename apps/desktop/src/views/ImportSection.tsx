import { useState } from "react";
import { api, ApiError } from "../lib/api";
import type { ImportResult } from "../lib/types";
import { useToast } from "../components/Toast";

function plural(n: number, one: string, many: string) {
  return `${n} ${n === 1 ? one : many}`;
}

/** Lines describing what was left out or changed; empty when nothing was. */
function caveats(r: ImportResult["report"]): string[] {
  const out: string[] = [];
  if (r.convertedToNotes) out.push(`${plural(r.convertedToNotes, "item", "items")} of other kinds (cards, identities, keys…) became secure notes with all their fields.`);
  if (r.skippedDuplicates) out.push(`${plural(r.skippedDuplicates, "item was", "items were")} already in your vault and skipped.`);
  if (r.skippedArchived) out.push(`${plural(r.skippedArchived, "archived item was", "archived items were")} left out.`);
  if (r.attachmentsSkipped) out.push(`${plural(r.attachmentsSkipped, "file attachment was", "file attachments were")} left out (not supported yet).`);
  if (r.passwordHistorySkipped) out.push(`${plural(r.passwordHistorySkipped, "old password", "old passwords")} from password history ${r.passwordHistorySkipped === 1 ? "was" : "were"} left out.`);
  if (r.urlsMovedToNotes) out.push(`${plural(r.urlsMovedToNotes, "website entry wasn’t", "website entries weren’t")} a web address and ${r.urlsMovedToNotes === 1 ? "was" : "were"} kept in the item’s notes.`);
  if (r.failed) out.push(`${plural(r.failed, "item", "items")} couldn’t be imported (a field was over the size limits).`);
  return out;
}

export function ImportSection({ onImported }: { onImported: () => void }) {
  const toast = useToast();
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
      toast(e instanceof ApiError ? e.message : "Import failed.", "error");
    } finally {
      setBusy(false);
    }
  }

  async function deleteFile() {
    try {
      await api.deleteImportFile();
      setDeleted(true);
      setConfirmDelete(false);
      toast("Export file deleted.");
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not delete the file.", "error");
    }
  }

  return (
    <div className="settings-block">
      <h3 className="group-title">Import from 1Password</h3>
      <p className="group-note group-note-top">
        In 1Password, choose File › Export and the 1PUX format, then pick that file here. The export contains all of your
        passwords unencrypted, so delete it once the import is done.
      </p>
      <div className="group-actions">
        <button className="btn" onClick={() => void runImport()} disabled={busy}>
          {busy ? "Importing…" : "Choose .1pux file…"}
        </button>
      </div>

      {result && (
        <div className="import-result" role="status">
          <p>
            <strong>
              Imported {plural(result.report.imported, "item", "items")}
            </strong>{" "}
            from {result.fileName}: {plural(result.report.logins, "login", "logins")} and{" "}
            {plural(result.report.secureNotes, "secure note", "secure notes")}.
          </p>
          {caveats(result.report).length > 0 && (
            <ul>
              {caveats(result.report).map((c) => (
                <li key={c}>{c}</li>
              ))}
            </ul>
          )}
          {deleted ? (
            <p className="muted">The export file was deleted.</p>
          ) : confirmDelete ? (
            <div className="confirm">
              <span>Delete {result.fileName}?</span>
              <button className="btn btn-small btn-danger" onClick={() => void deleteFile()}>
                Delete file
              </button>
              <button className="btn btn-small" onClick={() => setConfirmDelete(false)}>
                Keep it
              </button>
            </div>
          ) : (
            <div>
              <button className="btn btn-small" onClick={() => setConfirmDelete(true)}>
                Delete the export file
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

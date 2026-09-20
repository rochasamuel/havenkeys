import { useCallback, useEffect, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { DeviceStatus, SyncStatus } from "../lib/types";
import { formatDate } from "../lib/format";
import { useToast } from "../components/Toast";
import { EmergencyKit } from "../components/EmergencyKit";

function describe(s: SyncStatus | null): string {
  if (!s) return "Not synced yet on this run of HavenKeys.";
  if (!s.ok) return `Last attempt ${formatDate(s.at)} failed: ${s.error ?? "unknown error"}`;
  const r = s.report;
  if (!r) return `Synced ${formatDate(s.at)}.`;
  const changes = [
    r.added && `${r.added} added`,
    r.updated && `${r.updated} updated`,
    r.deleted && `${r.deleted} deleted`,
  ].filter(Boolean);
  const notes = [
    r.headerAdopted && "master password changed on another device (use the new one next time)",
    (r.unreadableDevices > 0 || r.headerRejected) && "some files in the folder were not valid and were ignored",
  ].filter(Boolean);
  return [`Synced ${formatDate(s.at)}.`, changes.length ? changes.join(", ") + "." : "No changes.", ...notes]
    .join(" ")
    .trim();
}

/** Add a Secret Key to a vault created before it existed. */
function AddSecretKey({ onDone }: { onDone: () => void }) {
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || !password) return;
    setBusy(true);
    setError(null);
    try {
      await api.setupSecretKey(password);
      setPassword("");
      onDone();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not add a Secret Key.");
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit} className="settings-inline">
      <p className="muted">
        This vault is protected by your master password only. Adding a Secret Key means a copy of the vault (in a sync
        folder or a backup) cannot be opened with the password alone. It is required for sync.
      </p>
      <label className="control">
        <span>Master password</span>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="off"
          spellCheck={false}
          disabled={busy}
        />
      </label>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <div>
        <button className="btn btn-primary" type="submit" disabled={busy || !password}>
          {busy ? "Adding…" : "Add a Secret Key"}
        </button>
      </div>
    </form>
  );
}

export function SyncSection() {
  const toast = useToast();
  const [device, setDevice] = useState<DeviceStatus | null>(null);
  const [kit, setKit] = useState<"hidden" | "shown" | "first">("hidden");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(() => {
    api.deviceStatus().then(setDevice, () => toast("Could not read sync settings.", "error"));
  }, [toast]);
  useEffect(refresh, [refresh]);

  async function act(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast(ok);
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Something went wrong.", "error");
    } finally {
      setBusy(false);
      refresh();
    }
  }

  if (!device) return null;
  const hasKey = device.usesSecretKey;
  // Folder sync is key scheme 2 only; an account-bound vault syncs through
  // its server instead (Rust refuses the folder picker for it).
  const canSyncThroughFolder = device.keyScheme === "password_and_secret_key";

  return (
    <div className="settings-block">
      <h3>Secret Key &amp; sync</h3>

      {!hasKey && kit === "hidden" && (
        <AddSecretKey
          onDone={() => {
            setKit("first");
            refresh();
          }}
        />
      )}

      {hasKey && kit === "hidden" && (
        <div>
          <button className="btn" type="button" onClick={() => setKit("shown")}>
            Show Emergency Kit
          </button>
        </div>
      )}
      {kit !== "hidden" && (
        <EmergencyKit onDone={() => setKit("hidden")} />
      )}

      {canSyncThroughFolder && (
        <>
          <p className="muted">
            Sync keeps your vault the same on your computers (and, later, the mobile app) through a folder that OneDrive,
            Dropbox, Google Drive or Syncthing already keeps in step. HavenKeys writes only encrypted files there; they
            cannot be opened without your master password and Secret Key. No HavenKeys server is involved.
          </p>
          <p>
            {device.syncFolder ? (
              <>
                Syncing through <strong>{device.syncFolder}</strong>.
              </>
            ) : (
              "Not syncing."
            )}
          </p>
          {device.syncFolder && <p className="muted">{describe(device.lastSync)}</p>}
          <div className="button-row">
            <button
              className="btn"
              type="button"
              disabled={busy}
              onClick={() =>
                void act(async () => {
                  const name = await api.chooseSyncFolder();
                  if (name) await api.syncNow();
                })
              }
            >
              {device.syncFolder ? "Change folder…" : "Choose sync folder…"}
            </button>
            {device.syncFolder && (
              <>
                <button className="btn" type="button" disabled={busy} onClick={() => void act(api.syncNow)}>
                  Sync now
                </button>
                <button
                  className="btn btn-quiet-danger"
                  type="button"
                  disabled={busy}
                  onClick={() => void act(api.stopSync, "Sync stopped on this computer.")}
                >
                  Stop syncing
                </button>
              </>
            )}
          </div>
          <p className="muted">
            On another computer, install HavenKeys, choose “Set up from a sync folder”, and enter your master password and
            the Secret Key from your Emergency Kit.
          </p>
        </>
      )}
    </div>
  );
}

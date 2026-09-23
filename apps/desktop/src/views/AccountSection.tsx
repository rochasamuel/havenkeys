import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { AccountStatus, DeviceEntry } from "../lib/types";
import { useToast } from "../components/Toast";
import { EmergencyKit } from "../components/EmergencyKit";

/**
 * Settings → Account: which account this vault belongs to, which computers
 * are signed in to it, and the Emergency Kit.
 *
 * The device list comes from the server, so it is only available while this
 * device has a session. Offline, the section still shows the account and the
 * kit, because both are local.
 */

function relative(iso: string | null): string {
  if (!iso) return "never";
  const at = Date.parse(iso);
  if (Number.isNaN(at)) return "unknown";
  const seconds = Math.max(0, Math.round((Date.now() - at) / 1000));
  if (seconds < 90) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 90) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 36) return `${hours} h ago`;
  return `${Math.round(hours / 24)} days ago`;
}

function Devices({ online }: { online: boolean }) {
  const toast = useToast();
  const [devices, setDevices] = useState<DeviceEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    if (!online) {
      setDevices(null);
      return;
    }
    api.listDevices().then(
      (list) => {
        setDevices(list);
        setError(null);
      },
      (e) => setError(e instanceof ApiError ? e.message : "Could not read the device list."),
    );
  }, [online]);
  useEffect(load, [load]);

  async function revoke(device: DeviceEntry) {
    setBusy(true);
    try {
      await api.revokeDevice(device.id);
      toast(device.current ? "This computer was signed out." : `${device.name} was signed out.`);
      setConfirming(null);
      load();
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not revoke that device.", "error");
    } finally {
      setBusy(false);
    }
  }

  if (!online) {
    return <p className="group-note">Your devices are listed when this computer is connected to the server.</p>;
  }
  if (error) return <p className="form-error">{error}</p>;
  if (!devices) return <p className="muted">Loading…</p>;

  return (
    <ul className="group device-list">
      {devices.map((device) => (
        <li key={device.id} className="device-row">
          <div className="device-main">
            <span className="device-name">
              {device.name}
              {device.current && <span className="pill">This computer</span>}
            </span>
            <span className="device-meta">Last seen {relative(device.lastSeenAt)}</span>
          </div>
          {confirming === device.id ? (
            <div className="device-confirm">
              <span className="muted">
                {device.current ? "Sign this computer out?" : "Sign it out and end its session?"}
              </span>
              <button className="btn btn-danger" type="button" disabled={busy} onClick={() => revoke(device)}>
                Revoke
              </button>
              <button className="btn btn-quiet" type="button" onClick={() => setConfirming(null)}>
                Cancel
              </button>
            </div>
          ) : (
            <button className="btn btn-quiet" type="button" onClick={() => setConfirming(device.id)}>
              Revoke
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}

export function AccountSection({ online }: { online: boolean }) {
  const toast = useToast();
  const [account, setAccount] = useState<AccountStatus | null>(null);
  const [showKit, setShowKit] = useState(false);
  const [syncing, setSyncing] = useState(false);

  const refresh = useCallback(() => {
    api.accountStatus().then(setAccount, () => setAccount(null));
  }, []);
  useEffect(refresh, [refresh, online]);

  async function sync() {
    setSyncing(true);
    try {
      const report = await api.syncNow();
      const changed = report.added + report.updated + report.deleted;
      toast(changed === 0 ? "Already up to date." : `Synced: ${changed} item(s) updated.`);
      if (report.skippedItems > 0) {
        toast(`${report.skippedItems} item(s) from the server could not be read and were left alone.`, "error");
      }
      refresh();
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not sync.", "error");
    } finally {
      setSyncing(false);
    }
  }

  async function resync() {
    setSyncing(true);
    try {
      const report = await api.resync();
      toast(`Re-downloaded the vault: ${report.added + report.updated} item(s).`);
      if (report.skippedItems > 0) {
        toast(`${report.skippedItems} item(s) still could not be read.`, "error");
      }
      refresh();
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not re-download the vault.", "error");
    } finally {
      setSyncing(false);
    }
  }

  async function signOut() {
    try {
      await api.signOut();
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not sign out.", "error");
    }
  }

  return (
    <div className="settings-block">
      <h3 className="group-title">Account</h3>

      {account ? (
        <dl className="group kv">
          <div>
            <dt>Email</dt>
            <dd>{account.email}</dd>
          </div>
          <div>
            <dt>Server</dt>
            <dd className="mono">{account.serverUrl}</dd>
          </div>
          <div>
            <dt>Status</dt>
            <dd>
              <span className={online ? "status status-ok" : "status status-muted"}>
                {online ? "Connected" : "Offline — read-only"}
              </span>
            </dd>
          </div>
          <div>
            <dt>Last sync</dt>
            <dd>{account.lastSyncedAt ? new Date(account.lastSyncedAt).toLocaleString() : "never"}</dd>
          </div>
        </dl>
      ) : (
        <p className="group-note">This vault is not linked to an account.</p>
      )}

      <div className="row-actions">
        <button className="btn" type="button" onClick={sync} disabled={!online || syncing}>
          {syncing ? "Syncing…" : "Sync now"}
        </button>
        <button className="btn btn-quiet" type="button" onClick={resync} disabled={!online || syncing}>
          Re-download everything
        </button>
        <button className="btn btn-quiet" type="button" onClick={signOut}>
          Sign out and lock
        </button>
      </div>

      <h3 className="group-title">Devices</h3>
      <Devices online={online} />

      <h3 className="group-title">Emergency Kit</h3>
      {!showKit ? (
        <div className="kit-teaser">
          <p className="muted">
            Your Secret Key, the account and the server — everything another computer needs, besides your master
            password.
          </p>
          <button className="btn" type="button" onClick={() => setShowKit(true)}>
            Show Emergency Kit
          </button>
        </div>
      ) : (
        <EmergencyKit onDone={() => setShowKit(false)} />
      )}
    </div>
  );
}

import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { AccountStatus, DeviceEntry, DeviceStatus } from "../lib/types";
import { useToast } from "../components/Toast";
import { EmergencyKit } from "../components/EmergencyKit";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import type { Messages } from "../i18n/en";

/**
 * Settings → Account: which account this vault belongs to, which computers
 * are signed in to it, and the Emergency Kit.
 *
 * The device list comes from the server, so it is only available while this
 * device has a session. Offline, the section still shows the account and the
 * kit, because both are local.
 */

function relative(iso: string | null, t: Messages): string {
  if (!iso) return t.account.never;
  const at = Date.parse(iso);
  if (Number.isNaN(at)) return t.account.unknown;
  const seconds = Math.max(0, Math.round((Date.now() - at) / 1000));
  if (seconds < 90) return t.account.justNow;
  const minutes = Math.round(seconds / 60);
  if (minutes < 90) return t.account.minutesAgo(minutes);
  const hours = Math.round(minutes / 60);
  if (hours < 36) return t.account.hoursAgo(hours);
  return t.account.daysAgo(Math.round(hours / 24));
}

/** The name of the device that approved this one, when it is still in the list. */
function approver(device: DeviceEntry, all: DeviceEntry[]): string | undefined {
  if (!device.approvedBy) return undefined;
  return all.find((d) => d.id === device.approvedBy)?.name;
}

function Devices({ online }: { online: boolean }) {
  const toast = useToast();
  const { t } = useI18n();
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
      (e) => setError(errorMessage(e, t, t.account.devicesFailed)),
    );
    // `t` is left out: a language change must not reload the list.
  }, [online]);
  useEffect(load, [load]);

  async function revoke(device: DeviceEntry) {
    setBusy(true);
    try {
      await api.revokeDevice(device.id);
      toast(device.current ? t.account.thisSignedOut : t.account.deviceSignedOut(device.name));
      setConfirming(null);
      load();
    } catch (e) {
      toast(errorMessage(e, t, t.account.revokeFailed), "error");
    } finally {
      setBusy(false);
    }
  }

  if (!online) {
    return <p className="group-note">{t.account.devicesOffline}</p>;
  }
  if (error) return <p className="form-error">{error}</p>;
  if (!devices) return <p className="muted">{t.common.loading}</p>;

  return (
    <ul className="group device-list">
      {devices.map((device) => (
        <li key={device.id} className="device-row">
          <div className="device-main">
            <span className="device-name">
              {device.name}
              {device.current && <span className="pill">{t.account.thisComputer}</span>}
            </span>
            <span className="device-meta">{t.account.lastSeen(relative(device.lastSeenAt, t))}</span>
            {approver(device, devices) && (
              <span className="device-meta">{t.account.approvedBy(approver(device, devices) ?? "")}</span>
            )}
          </div>
          {confirming === device.id ? (
            <div className="device-confirm">
              <span className="muted">
                {device.current ? t.account.confirmSignOutThis : t.account.confirmSignOutOther}
              </span>
              <button className="btn btn-danger" type="button" disabled={busy} onClick={() => revoke(device)}>
                {t.account.revoke}
              </button>
              <button className="btn btn-quiet" type="button" onClick={() => setConfirming(null)}>
                {t.common.cancel}
              </button>
            </div>
          ) : (
            <button className="btn btn-quiet" type="button" onClick={() => setConfirming(device.id)}>
              {t.account.revoke}
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}

export function AccountSection({ online }: { online: boolean }) {
  const toast = useToast();
  const { t, locale } = useI18n();
  const [account, setAccount] = useState<AccountStatus | null>(null);
  const [storage, setStorage] = useState<DeviceStatus["secretKeyStorage"] | null>(null);
  const [showKit, setShowKit] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [confirmText, setConfirmText] = useState("");
  const [removing, setRemoving] = useState(false);

  const refresh = useCallback(() => {
    api.accountStatus().then(setAccount, () => setAccount(null));
    api.deviceStatus().then(
      (d) => setStorage(d.secretKeyStorage),
      () => setStorage(null),
    );
  }, []);
  useEffect(refresh, [refresh, online]);

  async function sync() {
    setSyncing(true);
    try {
      const report = await api.syncNow();
      const changed = report.added + report.updated + report.deleted;
      toast(changed === 0 ? t.account.upToDate : t.account.synced(changed));
      if (report.skippedItems > 0) {
        toast(t.account.skipped(report.skippedItems), "error");
      }
      refresh();
    } catch (e) {
      toast(errorMessage(e, t, t.account.syncFailed), "error");
    } finally {
      setSyncing(false);
    }
  }

  async function resync() {
    setSyncing(true);
    try {
      const report = await api.resync();
      toast(t.account.redownloaded(report.added + report.updated));
      if (report.skippedItems > 0) {
        toast(t.account.stillSkipped(report.skippedItems), "error");
      }
      refresh();
    } catch (e) {
      toast(errorMessage(e, t, t.account.redownloadFailed), "error");
    } finally {
      setSyncing(false);
    }
  }

  async function signOut() {
    try {
      await api.signOut();
    } catch (e) {
      toast(errorMessage(e, t, t.account.signOutFailed), "error");
    }
  }

  async function remove() {
    setRemoving(true);
    try {
      await api.removeDevice(confirmText);
      // The vault reopened empty and the app returns to first run; nothing
      // left to refresh here.
    } catch (e) {
      toast(errorMessage(e, t, t.account.removeFailed), "error");
      setRemoving(false);
      // Belt and braces: the command can fail after the file was already
      // renamed aside (the rest of removal still ran — see remove_device),
      // so this section's own view of the account may be stale even though
      // an error came back. Re-reading it here does not depend on the
      // vault://removed listener in App.tsx also having run.
      refresh();
    }
  }

  return (
    <div className="settings-block">
      <h3 className="group-title">{t.account.title}</h3>

      {account ? (
        <dl className="group kv">
          <div>
            <dt>{t.common.email}</dt>
            <dd>{account.email}</dd>
          </div>
          <div>
            <dt>{t.common.server}</dt>
            <dd className="mono">{account.serverUrl}</dd>
          </div>
          <div>
            <dt>{t.account.status}</dt>
            <dd>
              <span className={online ? "status status-ok" : "status status-muted"}>
                {online ? t.account.connected : t.account.offline}
              </span>
            </dd>
          </div>
          <div>
            <dt>{t.account.lastSync}</dt>
            <dd>{account.lastSyncedAt ? new Date(account.lastSyncedAt).toLocaleString(locale) : t.account.never}</dd>
          </div>
        </dl>
      ) : (
        <p className="group-note">{t.account.notLinked}</p>
      )}

      <div className="row-actions">
        <button className="btn" type="button" onClick={sync} disabled={!online || syncing}>
          {syncing ? t.account.syncing : t.account.syncNow}
        </button>
        <button className="btn btn-quiet" type="button" onClick={resync} disabled={!online || syncing}>
          {t.account.redownloadAll}
        </button>
        <button className="btn btn-quiet" type="button" onClick={signOut}>
          {t.account.signOut}
        </button>
      </div>

      <h3 className="group-title">{t.account.devices}</h3>
      <Devices online={online} />

      <h3 className="group-title" id="emergency-kit">{t.account.kitTitle}</h3>
      {storage === "file" && (
        <p className="group-note warn">{t.account.keyInFile}</p>
      )}
      {!showKit ? (
        <div className="kit-teaser">
          <p className="muted">{t.account.kitTeaser}</p>
          <button className="btn" type="button" onClick={() => setShowKit(true)}>
            {t.account.showKit}
          </button>
        </div>
      ) : (
        <EmergencyKit onDone={() => setShowKit(false)} />
      )}

      <h3 className="group-title">{t.account.removeTitle}</h3>
      <div className="group danger-zone">
        <p className="group-note group-note-top">{t.account.removeNote}</p>
        <label className="row row-input">
          <span className="row-label-inline">{t.account.typeToConfirm(account?.email ?? "")}</span>
          <input
            value={confirmText}
            onChange={(e) => setConfirmText(e.target.value)}
            autoComplete="off"
            spellCheck={false}
          />
        </label>
      </div>
      <div className="group-actions">
        <button
          className="btn btn-danger"
          type="button"
          disabled={!confirmText || removing}
          onClick={remove}
        >
          {t.account.remove}
        </button>
      </div>
    </div>
  );
}

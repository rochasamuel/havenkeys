import { useEffect, useState } from "react";
import { api } from "./lib/api";
import type { DeviceStatus, VaultStatus } from "./lib/types";
import { useActivityReporter } from "./lib/hooks";
import { applyTheme } from "./lib/theme";
import { UnlockScreen } from "./views/UnlockScreen";
import { VaultScreen } from "./views/VaultScreen";

export function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [lockReason, setLockReason] = useState<string | null>(null);
  // Bumped on every lock so the whole vault tree (and any secret held in
  // component state) is unmounted and discarded.
  const [session, setSession] = useState(0);
  const [fatal, setFatal] = useState(false);
  const [device, setDevice] = useState<DeviceStatus | null>(null);

  useEffect(() => {
    api.status().then(setStatus, () => setFatal(true));
    const unlisten = api.onLocked((reason) => {
      setLockReason(reason);
      setSession((s) => s + 1);
      setStatus((s) => (s ? { ...s, state: "locked", damagedItems: 0 } : s));
    });
    return () => void unlisten.then((f) => f());
  }, []);

  const unlocked = status?.state === "unlocked";
  useActivityReporter(unlocked);

  // Device status (Secret Key requirement, online state) is safe to read at
  // any time, locked or not, so the offline banner can show right away.
  useEffect(() => {
    api.deviceStatus().then(setDevice, () => setDevice(null));
  }, [unlocked, session]);

  // The theme lives in the encrypted settings; apply it once they're readable.
  useEffect(() => {
    if (!unlocked) return;
    api.getSettings().then((s) => applyTheme(s.theme), () => undefined);
  }, [unlocked]);

  // Ctrl/Cmd+L locks from anywhere.
  useEffect(() => {
    if (!unlocked) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "l") {
        e.preventDefault();
        void api.lock();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [unlocked]);

  if (fatal) {
    return (
      <main className="unlock">
        <p className="unlock-error">HavenKeys could not reach its vault storage. Restart the app.</p>
      </main>
    );
  }
  if (!status) return <main className="unlock" aria-busy="true" />;

  if (!status.vaultExists) {
    return (
      <div className="empty-state">
        <h1>No vault on this computer</h1>
        <p>
          HavenKeys vaults belong to an account. Activation with an invite is not available in this build yet.
        </p>
      </div>
    );
  }

  if (!unlocked) {
    return (
      <UnlockScreen
        key={session}
        lockReason={lockReason}
        needsSecretKey={device?.needsSecretKey ?? false}
        onUnlocked={(s) => {
          setLockReason(null);
          setStatus(s);
        }}
      />
    );
  }

  return (
    <div className="app-shell">
      {device?.online === false && (
        <div className="banner banner-muted" role="status">
          Offline — the vault is read-only until it reconnects.
        </div>
      )}
      <VaultScreen
        key={session}
        damagedItems={status.damagedItems}
        readOnly={!device?.online}
        onLock={() => {
          setLockReason("user");
          setSession((s) => s + 1);
          setStatus({ ...status, state: "locked", damagedItems: 0 });
        }}
      />
    </div>
  );
}

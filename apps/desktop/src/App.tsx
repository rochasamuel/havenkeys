import { useEffect, useState } from "react";
import { api } from "./lib/api";
import type { DeviceStatus, VaultStatus } from "./lib/types";
import { useActivityReporter } from "./lib/hooks";
import { applyTheme } from "./lib/theme";
import { EmergencyKit } from "./components/EmergencyKit";
import { UnlockScreen } from "./views/UnlockScreen";
import { VaultScreen } from "./views/VaultScreen";
import { WelcomeScreen } from "./views/WelcomeScreen";

export function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [lockReason, setLockReason] = useState<string | null>(null);
  // Bumped on every lock so the whole vault tree (and any secret held in
  // component state) is unmounted and discarded.
  const [session, setSession] = useState(0);
  const [fatal, setFatal] = useState(false);
  const [device, setDevice] = useState<DeviceStatus | null>(null);
  // Shown once, right after activation: the kit is the only copy of the
  // Secret Key, so the vault waits behind an explicit confirmation.
  const [showKit, setShowKit] = useState(false);

  useEffect(() => {
    api.status().then(setStatus, () => setFatal(true));
    const unlisten = api.onLocked((reason) => {
      setLockReason(reason);
      setSession((s) => s + 1);
      setShowKit(false);
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

  // The session is opened in the background after an unlock, and can be lost
  // at any time; the banner and the read-only state follow it live.
  useEffect(() => {
    const unlisten = api.onConnectivity((online) => {
      setDevice((d) => (d ? { ...d, online } : d));
    });
    return () => void unlisten.then((f) => f());
  }, []);

  // The theme lives in the encrypted settings; apply it once they're readable.
  useEffect(() => {
    if (!unlocked) return;
    api.getSettings().then(
      (s) => applyTheme(s.theme),
      () => undefined,
    );
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
      <WelcomeScreen
        onActivated={(s) => {
          setStatus(s);
          setShowKit(true);
        }}
        onSignedIn={setStatus}
      />
    );
  }

  if (showKit && unlocked) {
    return (
      <main className="kit-screen">
        <header className="kit-screen-head">
          <h1>Save your Emergency Kit</h1>
          <p>
            This is the only copy of your Secret Key. Without it — and your master password — nobody can open this
            vault, including us.
          </p>
        </header>
        <EmergencyKit onDone={() => setShowKit(false)} />
      </main>
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

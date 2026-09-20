import { useEffect, useState } from "react";
import { api } from "./lib/api";
import type { DeviceStatus, VaultStatus } from "./lib/types";
import { EmergencyKit } from "./components/EmergencyKit";
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
  // Right after creating a vault: show the Emergency Kit before the vault.
  const [showKit, setShowKit] = useState(false);

  useEffect(() => {
    api.status().then(setStatus, () => setFatal(true));
    const unlisten = api.onLocked((reason) => {
      setLockReason(reason);
      setShowKit(false);
      setSession((s) => s + 1);
      setStatus((s) => (s ? { ...s, state: "locked", damagedItems: 0 } : s));
    });
    return () => void unlisten.then((f) => f());
  }, []);

  const unlocked = status?.state === "unlocked";
  useActivityReporter(unlocked);

  // Whether this device must be given the Secret Key (safe while locked).
  useEffect(() => {
    if (unlocked) return;
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

  if (!unlocked) {
    return (
      <UnlockScreen
        key={session}
        mode={status.vaultExists ? "unlock" : "create"}
        lockReason={lockReason}
        needsSecretKey={device?.needsSecretKey ?? false}
        usesSecretKey={device?.usesSecretKey ?? false}
        onUnlocked={(s, created) => {
          setLockReason(null);
          setShowKit(created);
          setStatus(s);
        }}
      />
    );
  }

  if (showKit) {
    return (
      <main className="kit-screen">
        <div className="kit-intro">
          <h1>Save your Emergency Kit</h1>
          <p>
            Your vault is protected by your master password <em>and</em> a Secret Key created just now. This computer
            remembers the Secret Key. You will need it to open your vault anywhere else, or on this computer if its
            HavenKeys data is lost.
          </p>
        </div>
        <EmergencyKit key={session} onDone={() => setShowKit(false)} />
      </main>
    );
  }

  return (
    <VaultScreen
      key={session}
      damagedItems={status.damagedItems}
      onLock={() => {
        setLockReason("user");
        setSession((s) => s + 1);
        setStatus({ ...status, state: "locked", damagedItems: 0 });
      }}
    />
  );
}

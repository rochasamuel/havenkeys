import { useEffect, useState } from "react";
import { api, ApiError } from "./lib/api";
import type { DeviceStatus, VaultStatus } from "./lib/types";
import { useActivityReporter } from "./lib/hooks";
import { applyTheme } from "./lib/theme";
import { EmergencyKit } from "./components/EmergencyKit";
import { Seal } from "./components/Seal";
import { UnlockScreen } from "./views/UnlockScreen";
import { VaultScreen } from "./views/VaultScreen";
import { WelcomeScreen } from "./views/WelcomeScreen";

export function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [lockReason, setLockReason] = useState<string | null>(null);
  // Bumped on every lock so the whole vault tree (and any secret held in
  // component state) is unmounted and discarded.
  const [session, setSession] = useState(0);
  // Why the vault could not be opened at all, in the core's own words: an
  // old vault file this build cannot read is the case that matters, and the
  // message names the folder it is in.
  const [fatal, setFatal] = useState<string | null>(null);
  const [device, setDevice] = useState<DeviceStatus | null>(null);
  // Shown once, right after activation: the kit is the only copy of the
  // Secret Key, so the vault waits behind an explicit confirmation.
  const [showKit, setShowKit] = useState(false);
  // The server refused this computer's sign-in (most often: the master
  // password was changed on another device). Replaces the offline banner
  // until the next lock or a successful reconnect.
  const [signedOut, setSignedOut] = useState(false);

  useEffect(() => {
    api.status().then(setStatus, (err) =>
      setFatal(
        err instanceof ApiError
          ? err.message
          : "HavenKeys could not reach its vault storage. Restart the app.",
      ),
    );
    const unlisten = api.onLocked((reason) => {
      setLockReason(reason);
      setSession((s) => s + 1);
      setShowKit(false);
      setSignedOut(false);
      setStatus((s) => (s ? { ...s, state: "locked", damagedItems: 0, unreadableItems: 0 } : s));
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
      if (online) setSignedOut(false);
    });
    const unlistenSignedOut = api.onSignedOut(() => setSignedOut(true));
    return () => {
      void unlisten.then((f) => f());
      void unlistenSignedOut.then((f) => f());
    };
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
      <main className="welcome" data-tauri-drag-region>
        <div className="welcome-card">
          <header className="welcome-head">
            <Seal />
            <h1>HavenKeys cannot open this vault</h1>
          </header>
          <p className="fatal-message">{fatal}</p>
        </div>
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
        <header className="kit-screen-head" data-tauri-drag-region>
          <Seal size={48} />
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
      {signedOut ? (
        <div className="banner" role="status">
          The server did not accept this computer&apos;s sign-in. If your master password was changed on another
          device, lock and unlock with the new one.
        </div>
      ) : (
        device?.online === false && (
          <div className="banner" role="status">
            Offline — the vault is read-only until it reconnects.
          </div>
        )
      )}
      <VaultScreen
        key={session}
        damagedItems={status.damagedItems}
        unreadableItems={status.unreadableItems}
        readOnly={!device?.online}
        onLock={() => {
          setLockReason("user");
          setSignedOut(false);
          setSession((s) => s + 1);
          setStatus({ ...status, state: "locked", damagedItems: 0, unreadableItems: 0 });
        }}
      />
    </div>
  );
}

import { useEffect, useState } from "react";
import { api } from "./lib/api";
import type { DeviceStatus, VaultStatus } from "./lib/types";
import { useActivityReporter } from "./lib/hooks";
import { applyTheme } from "./lib/theme";
import { EmergencyKit } from "./components/EmergencyKit";
import { Seal } from "./components/Seal";
import { UnlockScreen } from "./views/UnlockScreen";
import { VaultScreen } from "./views/VaultScreen";
import { WelcomeScreen } from "./views/WelcomeScreen";
import { useI18n } from "./i18n/context";
import { errorMessage } from "./i18n/errors";

export function App() {
  const { t } = useI18n();
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [lockReason, setLockReason] = useState<string | null>(null);
  // Bumped on every lock so the whole vault tree (and any secret held in
  // component state) is unmounted and discarded.
  const [session, setSession] = useState(0);
  // Why the vault could not be opened at all, in the core's own words: an
  // old vault file this build cannot read is the case that matters, and the
  // message names the folder it is in.
  const [fatal, setFatal] = useState<{ cause: unknown } | null>(null);
  const [device, setDevice] = useState<DeviceStatus | null>(null);
  // Shown once, right after activation: the kit is the only copy of the
  // Secret Key, so the vault waits behind an explicit confirmation.
  const [showKit, setShowKit] = useState(false);
  // The server refused this computer's sign-in (most often: the master
  // password was changed on another device). Replaces the offline banner
  // until the next lock or a successful reconnect.
  const [signedOut, setSignedOut] = useState(false);
  // Set by "Remove this device" when the system keychain would not confirm
  // that the Secret Key was deleted. Shown until dismissed. Rust's text is
  // fixed, so the UI shows its own translation of it.
  const [removedWarning, setRemovedWarning] = useState(false);

  useEffect(() => {
    api.status().then(setStatus, (err) =>
      // Kept as the error itself so its text follows the language. The
      // vault_unreadable message is Rust's own: it names the folder the file
      // is in, and only Rust knows it.
      setFatal({ cause: err }),
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

  // This computer was removed from its account: the store reopened empty, so
  // re-reading status shows the first-run screen (vaultExists: false).
  useEffect(() => {
    const unlisten = api.onRemoved(({ keychainWarning }) => {
      setRemovedWarning(keychainWarning !== null);
      setLockReason(null);
      setShowKit(false);
      setSignedOut(false);
      setSession((s) => s + 1);
      api.status().then(setStatus, () => undefined);
      api.deviceStatus().then(setDevice, () => setDevice(null));
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
            <h1>{t.app.fatalTitle}</h1>
          </header>
          <p className="fatal-message">{errorMessage(fatal.cause, t, t.app.fatalFallback)}</p>
        </div>
      </main>
    );
  }
  if (!status) return <main className="unlock" aria-busy="true" />;

  if (!status.vaultExists) {
    return (
      <>
        {removedWarning && (
          <div className="banner banner-warn banner-fixed" role="alert">
            <span>{t.app.keychainNotCleared}</span>
            <button className="btn btn-quiet" type="button" onClick={() => setRemovedWarning(false)}>
              {t.common.dismiss}
            </button>
          </div>
        )}
        <WelcomeScreen
          onActivated={(s) => {
            setRemovedWarning(false);
            setStatus(s);
            setShowKit(true);
          }}
          onSignedIn={(s) => {
            setRemovedWarning(false);
            setStatus(s);
          }}
        />
      </>
    );
  }

  if (showKit && unlocked) {
    return (
      <main className="kit-screen">
        <header className="kit-screen-head" data-tauri-drag-region>
          <Seal size={48} />
          <h1>{t.app.kitTitle}</h1>
          <p>{t.app.kitBody}</p>
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
          {t.app.signedOut}
        </div>
      ) : (
        device?.online === false && (
          <div className="banner" role="status">
            {t.common.offlineReadOnly}
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

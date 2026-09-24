import { useEffect, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { Settings, Theme } from "../lib/types";
import { applyTheme } from "../lib/theme";
import { Icon } from "../components/Icon";
import { Switch } from "../components/Switch";
import { useToast } from "../components/Toast";
import { ImportSection } from "./ImportSection";
import { AccountSection } from "./AccountSection";

const autoLockChoices = [
  { value: 5, label: "After 5 minutes" },
  { value: 15, label: "After 15 minutes" },
  { value: 30, label: "After 30 minutes" },
  { value: 60, label: "After 1 hour" },
  { value: 0, label: "Never" },
];
const clipboardChoices = [10, 20, 30, 60, 90, 120];

function ChangePassword() {
  const toast = useToast();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mismatch = confirm.length > 0 && confirm !== next;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy || mismatch || !current || !next) return;
    setBusy(true);
    setError(null);
    try {
      await api.changeMasterPassword(current, next);
      setCurrent("");
      setNext("");
      setConfirm("");
      toast("Master password changed.");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not change the master password.");
    } finally {
      setBusy(false);
    }
  }

  const input = (value: string, set: (v: string) => void, label: string) => (
    <label className="row row-input">
      <span className="row-label-inline">{label}</span>
      <input
        type="password"
        value={value}
        onChange={(e) => set(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        disabled={busy}
      />
    </label>
  );

  return (
    <form className="settings-block" onSubmit={submit}>
      <h3 className="group-title">Master password</h3>
      <div className="group">
        {input(current, setCurrent, "Current")}
        {input(next, setNext, "New")}
        {input(confirm, setConfirm, "Confirm new")}
      </div>
      <p className="group-note">
        Your items are not re-encrypted; only the key that protects them changes. Other computers signed in to this
        account will be signed out.
      </p>
      {mismatch && <p className="form-error">The new passwords don’t match.</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <div className="group-actions">
        <button className="btn btn-primary" type="submit" disabled={busy || mismatch || !current || !next || !confirm}>
          {busy ? "Changing…" : "Change master password"}
        </button>
      </div>
    </form>
  );
}

export function SettingsView({ onImported, online }: { onImported: () => void; online: boolean }) {
  const toast = useToast();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [launchAtLogin, setLaunchAtLogin] = useState<boolean | null>(null);

  useEffect(() => {
    api
      .getSettings()
      .then(setSettings)
      .catch(() => toast("Could not load settings.", "error"));
    // Unknown (null) hides the row: the OS may not support it.
    api
      .launchAtLogin()
      .then(setLaunchAtLogin)
      .catch(() => setLaunchAtLogin(null));
  }, [toast]);

  async function updateLaunchAtLogin(enabled: boolean) {
    try {
      setLaunchAtLogin(await api.setLaunchAtLogin(enabled));
      toast("Settings saved.");
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not save settings.", "error");
    }
  }

  async function update(patch: Partial<Settings>) {
    if (!settings) return;
    try {
      const saved = await api.updateSettings({ ...settings, ...patch });
      setSettings(saved);
      applyTheme(saved.theme);
      toast("Settings saved.");
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not save settings.", "error");
    }
  }

  return (
    <section className="tool" aria-labelledby="settings-title">
      <header className="tool-head" data-tauri-drag-region>
        <h2 id="settings-title">Settings</h2>
      </header>

      {settings && (
        <div className="settings-block">
          <h3 className="group-title">Appearance</h3>
          <div className="group">
            <div className="row">
              <span className="row-label-inline">Theme</span>
              <div className="segmented" role="radiogroup" aria-label="Theme">
                {(["dark", "light", "system"] as Theme[]).map((t) => (
                  <button
                    key={t}
                    role="radio"
                    aria-checked={settings.theme === t}
                    onClick={() => void update({ theme: t })}
                  >
                    {t === "dark" ? "Dark" : t === "light" ? "Light" : "System"}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      {settings && (
        <div className="settings-block">
          <h3 className="group-title">Security</h3>
          <div className="group">
            <label className="row">
              <span className="row-label-inline">Lock automatically</span>
              <span className="select-wrap">
                <select
                  value={settings.autoLockMinutes}
                  onChange={(e) => void update({ autoLockMinutes: Number(e.target.value) })}
                >
                  {autoLockChoices.map((c) => (
                    <option key={c.value} value={c.value}>
                      {c.label}
                    </option>
                  ))}
                </select>
                <Icon name="chevronDown" size={14} className="select-chevron" />
              </span>
            </label>
            <label className="row">
              <span className="row-label-inline">Clear copied items</span>
              <span className="select-wrap">
                <select
                  value={settings.clipboardClearSeconds}
                  onChange={(e) => void update({ clipboardClearSeconds: Number(e.target.value) })}
                >
                  {clipboardChoices.map((s) => (
                    <option key={s} value={s}>
                      After {s} seconds
                    </option>
                  ))}
                </select>
                <Icon name="chevronDown" size={14} className="select-chevron" />
              </span>
            </label>
          </div>
          <p className="group-note">
            The vault also locks when the computer sleeps and when you quit HavenKeys, and — on Windows and Linux — when
            the screen locks. Closing the window keeps HavenKeys in the tray, where the timeout above keeps running.
          </p>
        </div>
      )}

      {launchAtLogin !== null && (
        <div className="settings-block">
          <h3 className="group-title">Startup</h3>
          <div className="group">
            <div className="row">
              <span className="row-label-inline">Open HavenKeys when you log in</span>
              <Switch
                label="Open HavenKeys when you log in to this computer"
                checked={launchAtLogin}
                onChange={(checked) => void updateLaunchAtLogin(checked)}
              />
            </div>
          </div>
          <p className="group-note">
            HavenKeys starts locked, in the tray, so the browser extension can reach it. This applies to this computer
            only.
          </p>
        </div>
      )}

      {settings && (
        <div className="settings-block">
          <h3 className="group-title">Browser extension</h3>
          <div className="group">
            <div className="row">
              <span className="row-label-inline">Suggest and fill logins in the browser</span>
              <Switch
                label="Allow the HavenKeys browser extension to suggest and fill logins"
                checked={settings.browserIntegration}
                onChange={(checked) => void update({ browserIntegration: checked })}
              />
            </div>
            <div className="row">
              <span className="row-label-inline">Add passkeys automatically after I sign in</span>
              <Switch
                label="Add passkeys automatically after I sign in"
                checked={settings.autoPasskeyUpgrade}
                onChange={(checked) => void update({ autoPasskeyUpgrade: checked })}
              />
            </div>
          </div>
          <p className="group-note">
            The extension only receives a login when you choose it on a website that login is saved for, and only while
            HavenKeys is unlocked. It never receives your master password. Off by default: while it is on, other programs
            running under your account can make the same requests as the extension.
          </p>
          <p className="group-note">
            When a website offers to add a passkey right after HavenKeys fills your password there, HavenKeys saves it to
            that login. When off, HavenKeys asks first.
          </p>
        </div>
      )}

      <AccountSection online={online} />

      <ImportSection onImported={onImported} />

      <ChangePassword />

      <div className="settings-block">
        <h3 className="group-title">About</h3>
        <p className="group-note">
          HavenKeys 0.4.0. Your vault is encrypted with AES-256-GCM under a key derived from your master password (with
          Argon2id) and your Secret Key. It stays on this computer unless you turn on sync. This software has not
          undergone an independent security audit.
        </p>
      </div>
    </section>
  );
}

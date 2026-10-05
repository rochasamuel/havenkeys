import { useEffect, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import type { Settings, Theme } from "../lib/types";
import { applyTheme } from "../lib/theme";
import { Icon } from "../components/Icon";
import { Switch } from "../components/Switch";
import { useToast } from "../components/Toast";
import { ExportSection } from "./ExportSection";
import { ImportSection } from "./ImportSection";
import { AccountSection } from "./AccountSection";
import { UpdatesSection } from "./UpdatesSection";
import { useI18n } from "../i18n/context";
import { useUpdateStatus } from "../lib/hooks";
import { errorMessage } from "../i18n/errors";
import { isPreference, LANGUAGE_NAMES, PREFERENCES } from "../i18n/locale";

/** Minutes; 0 is never. */
const autoLockChoices = [5, 15, 30, 60, 0];
const clipboardChoices = [10, 20, 30, 60, 90, 120];

function ChangePassword() {
  const toast = useToast();
  const { t } = useI18n();
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
      toast(t.changePassword.done);
    } catch (err) {
      setError(errorMessage(err, t, t.changePassword.failed));
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
      <h3 className="group-title">{t.changePassword.title}</h3>
      <div className="group">
        {input(current, setCurrent, t.changePassword.current)}
        {input(next, setNext, t.changePassword.new)}
        {input(confirm, setConfirm, t.changePassword.confirm)}
      </div>
      <p className="group-note">{t.changePassword.note}</p>
      {mismatch && <p className="form-error">{t.changePassword.mismatch}</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <div className="group-actions">
        <button className="btn btn-primary" type="submit" disabled={busy || mismatch || !current || !next || !confirm}>
          {busy ? t.changePassword.submitting : t.changePassword.submit}
        </button>
      </div>
    </form>
  );
}

export function SettingsView({ onImported, online }: { onImported: () => void; online: boolean }) {
  const toast = useToast();
  const { t, preference, setPreference } = useI18n();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [launchAtLogin, setLaunchAtLogin] = useState<boolean | null>(null);
  const updateStatus = useUpdateStatus();

  useEffect(() => {
    api
      .getSettings()
      .then(setSettings)
      .catch(() => toast(t.settings.loadFailed, "error"));
    // Unknown (null) hides the row: the OS may not support it.
    api
      .launchAtLogin()
      .then(setLaunchAtLogin)
      .catch(() => setLaunchAtLogin(null));
    // `t` is left out: a language change must not reload the settings.
  }, [toast]);

  async function updateLaunchAtLogin(enabled: boolean) {
    try {
      setLaunchAtLogin(await api.setLaunchAtLogin(enabled));
      toast(t.settings.saved);
    } catch (e) {
      toast(errorMessage(e, t, t.settings.saveFailed), "error");
    }
  }

  async function update(patch: Partial<Settings>) {
    if (!settings) return;
    try {
      const saved = await api.updateSettings({ ...settings, ...patch });
      setSettings(saved);
      applyTheme(saved.theme);
      toast(t.settings.saved);
    } catch (e) {
      toast(errorMessage(e, t, t.settings.saveFailed), "error");
    }
  }

  return (
    <section className="tool" aria-labelledby="settings-title">
      <header className="tool-head" data-tauri-drag-region>
        <h2 id="settings-title">{t.settings.title}</h2>
      </header>

      <div className="settings-block">
        <h3 className="group-title">{t.settings.appearance}</h3>
        <div className="group">
          <label className="row">
            <span className="row-label-inline">{t.settings.language}</span>
            <span className="select-wrap">
              <select
                value={preference}
                onChange={(e) => {
                  if (isPreference(e.target.value)) setPreference(e.target.value);
                }}
              >
                {PREFERENCES.map((p) => (
                  <option key={p} value={p}>
                    {p === "auto" ? t.settings.languageAuto : LANGUAGE_NAMES[p]}
                  </option>
                ))}
              </select>
              <Icon name="chevronDown" size={14} className="select-chevron" />
            </span>
          </label>
          {settings && (
            <div className="row">
              <span className="row-label-inline">{t.settings.theme}</span>
              <div className="segmented" role="radiogroup" aria-label={t.settings.theme}>
                {(["dark", "light", "system"] as Theme[]).map((theme) => (
                  <button
                    key={theme}
                    role="radio"
                    aria-checked={settings.theme === theme}
                    onClick={() => void update({ theme })}
                  >
                    {t.settings.themes[theme]}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>

      {settings && (
        <div className="settings-block">
          <h3 className="group-title">{t.settings.security}</h3>
          <div className="group">
            <label className="row">
              <span className="row-label-inline">{t.settings.autoLock}</span>
              <span className="select-wrap">
                <select
                  value={settings.autoLockMinutes}
                  onChange={(e) => void update({ autoLockMinutes: Number(e.target.value) })}
                >
                  {autoLockChoices.map((minutes) => (
                    <option key={minutes} value={minutes}>
                      {minutes === 0 ? t.settings.never : t.settings.autoLockAfter(minutes)}
                    </option>
                  ))}
                </select>
                <Icon name="chevronDown" size={14} className="select-chevron" />
              </span>
            </label>
            <label className="row">
              <span className="row-label-inline">{t.settings.clipboardClear}</span>
              <span className="select-wrap">
                <select
                  value={settings.clipboardClearSeconds}
                  onChange={(e) => void update({ clipboardClearSeconds: Number(e.target.value) })}
                >
                  {clipboardChoices.map((s) => (
                    <option key={s} value={s}>
                      {t.settings.afterSeconds(s)}
                    </option>
                  ))}
                </select>
                <Icon name="chevronDown" size={14} className="select-chevron" />
              </span>
            </label>
          </div>
          <p className="group-note">{t.settings.securityNote}</p>
        </div>
      )}

      {launchAtLogin !== null && (
        <div className="settings-block">
          <h3 className="group-title">{t.settings.startup}</h3>
          <div className="group">
            <div className="row">
              <span className="row-label-inline">{t.settings.launchAtLogin}</span>
              <Switch
                label={t.settings.launchAtLoginLabel}
                checked={launchAtLogin}
                onChange={(checked) => void updateLaunchAtLogin(checked)}
              />
            </div>
          </div>
          <p className="group-note">{t.settings.startupNote}</p>
        </div>
      )}

      {settings && (
        <div className="settings-block">
          <h3 className="group-title">{t.settings.browserExtension}</h3>
          <div className="group">
            <div className="row">
              <span className="row-label-inline">{t.settings.browserIntegration}</span>
              <Switch
                label={t.settings.browserIntegrationLabel}
                checked={settings.browserIntegration}
                onChange={(checked) => void update({ browserIntegration: checked })}
              />
            </div>
            <div className="row">
              <span className="row-label-inline">{t.settings.autoPasskey}</span>
              <Switch
                label={t.settings.autoPasskey}
                checked={settings.autoPasskeyUpgrade}
                onChange={(checked) => void update({ autoPasskeyUpgrade: checked })}
              />
            </div>
            <div className="row">
              <span className="row-label-inline">{t.settings.autoSignIn}</span>
              <Switch
                label={t.settings.autoSignIn}
                checked={settings.autoSignIn}
                onChange={(checked) => void update({ autoSignIn: checked })}
              />
            </div>
          </div>
          <p className="group-note">{t.settings.extensionNote}</p>
          <p className="group-note">{t.settings.passkeyNote}</p>
          <p className="group-note">{t.settings.autoSignInNote}</p>
        </div>
      )}

      <AccountSection online={online} />

      <ImportSection onImported={onImported} />

      <ExportSection />

      <ChangePassword />

      <UpdatesSection />

      <div className="settings-block">
        <h3 className="group-title">{t.settings.about}</h3>
        {updateStatus && <p className="group-note">{t.settings.aboutText(updateStatus.currentVersion)}</p>}
      </div>
    </section>
  );
}

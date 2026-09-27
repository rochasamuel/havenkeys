import { useState } from "react";
import { api } from "../lib/api";
import { useUpdateStatus } from "../lib/hooks";
import { Switch } from "../components/Switch";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

/** Settings → Updates: this computer's automatic check, and "Check now". */
export function UpdatesSection() {
  const { t } = useI18n();
  const toast = useToast();
  const status = useUpdateStatus();
  // Only a check asked for here says "up to date" or "could not check".
  const [asked, setAsked] = useState(false);

  if (!status) return null;

  async function setAutoCheck(enabled: boolean) {
    try {
      await api.setUpdateAutoCheck(enabled);
      toast(t.settings.saved);
    } catch (e) {
      toast(errorMessage(e, t, t.settings.saveFailed), "error");
    }
  }

  async function checkNow() {
    setAsked(true);
    await api.checkForUpdate().catch(() => undefined);
  }

  const result = !asked
    ? null
    : status.phase === "checking"
      ? t.updates.checking
      : status.phase === "idle"
        ? t.updates.upToDate
        : status.phase === "failed" && status.during === "check"
          ? t.updates.checkFailed
          : null;

  return (
    <div className="settings-block">
      <h3 className="group-title">{t.updates.title}</h3>
      <div className="group">
        <div className="row">
          <span className="row-label-inline">{t.updates.autoCheck}</span>
          <Switch
            label={t.updates.autoCheck}
            checked={status.autoCheck}
            onChange={(checked) => void setAutoCheck(checked)}
          />
        </div>
        <div className="row">
          <span className="row-label-inline">{t.updates.version(status.currentVersion)}</span>
          <button
            className="btn btn-quiet"
            type="button"
            disabled={status.phase === "checking" || status.phase === "downloading" || status.phase === "installing"}
            onClick={() => void checkNow()}
          >
            {t.updates.checkNow}
          </button>
        </div>
      </div>
      {result && (
        <p className="group-note" role="status">
          {result}
        </p>
      )}
      <p className="group-note">{status.canInstallInPlace ? t.updates.note : t.updates.manualNote}</p>
    </div>
  );
}

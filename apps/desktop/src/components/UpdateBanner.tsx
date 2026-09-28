import { useState } from "react";
import { api } from "../lib/api";
import { useUpdateStatus } from "../lib/hooks";
import { bannerFor } from "../lib/updates";
import type { UpdateStatus } from "../lib/types";
import { useI18n } from "../i18n/context";

/**
 * "HavenKeys 0.9.1 is available" at the top of the window, locked or not.
 * Release notes are rendered as text only (see UpdateBanner.test.ts).
 * Failures are reported through the status, so rejected calls are ignored.
 */
export function UpdateBanner({
  dismissed,
  onDismiss,
}: {
  /** The version the user chose "Later" for; kept by App so unlocking does not bring it back. */
  dismissed: string | null;
  onDismiss: (version: string) => void;
}) {
  const { t } = useI18n();
  const status = useUpdateStatus();
  const [showNotes, setShowNotes] = useState(false);
  // The failed status the user chose "Later" for. Every status event is a
  // new object, so the next change (a new failure included) shows again.
  const [hiddenFailure, setHiddenFailure] = useState<UpdateStatus | null>(null);
  const banner = bannerFor(status, dismissed);

  switch (banner.kind) {
    case "hidden":
      return null;
    case "available":
      return (
        <div className="banner update-banner" role="status">
          <span>{t.updates.available(banner.version)}</span>
          {banner.action === "update" && <span className="update-hint">{t.updates.restartNote}</span>}
          {banner.notes && (
            <button className="btn btn-quiet" type="button" onClick={() => setShowNotes((v) => !v)}>
              {showNotes ? t.updates.hideNotes : t.updates.whatsNew}
            </button>
          )}
          <button className="btn btn-quiet" type="button" onClick={() => onDismiss(banner.version)}>
            {t.updates.later}
          </button>
          <button className="btn" type="button" onClick={() => void api.installUpdate().catch(() => undefined)}>
            {banner.action === "update" ? t.updates.update : t.updates.download}
          </button>
          {showNotes && banner.notes && <p className="update-notes">{banner.notes}</p>}
        </div>
      );
    case "downloading":
      return (
        <div className="banner update-banner" role="status">
          <span>{t.updates.downloading(banner.version, banner.percent)}</span>
          <progress className="update-progress" max={100} value={banner.percent ?? undefined} />
        </div>
      );
    case "installing":
      return (
        <div className="banner update-banner" role="status">
          {t.updates.installing}
        </div>
      );
    case "failed":
      if (status === hiddenFailure) return null;
      return (
        <div className="banner banner-warn update-banner" role="alert">
          <span>{t.updates.failed}</span>
          <button className="btn btn-quiet" type="button" onClick={() => setHiddenFailure(status)}>
            {t.updates.later}
          </button>
          <button className="btn btn-quiet" type="button" onClick={() => void api.checkForUpdate().catch(() => undefined)}>
            {t.updates.tryAgain}
          </button>
        </div>
      );
  }
}

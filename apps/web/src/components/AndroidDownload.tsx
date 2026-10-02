import { useEffect, useState } from "react";
import { Icon } from "./Icon";
import { useI18n } from "../i18n/context";
import {
  ANDROID_CERT_SHA256,
  fetchLatestAndroidRelease,
  pickAndroidAssets,
  RELEASES_PAGE_URL,
  type LatestRelease,
} from "../lib/releases";

type State = { status: "loading" } | { status: "ready"; release: LatestRelease } | { status: "none" };

export function AndroidDownload({ yours }: { yours: boolean }) {
  const { t } = useI18n();
  const a = t.download.android;
  const [state, setState] = useState<State>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    fetchLatestAndroidRelease().then((release) => {
      if (!cancelled) setState(release ? { status: "ready", release } : { status: "none" });
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const assets = state.status === "ready" ? pickAndroidAssets(state.release.assets) : null;
  const version = state.status === "ready" ? state.release.tag_name.replace(/^android-v/, "") : null;

  return (
    <div className={`platform platform--android ${yours ? "platform--yours" : ""}`}>
      <div className="platform__text">
        <h2>{a.label}</h2>
        <p>{a.format}</p>
        <span className="tag">{a.early}</span>
        {yours && <span className="tag">{t.download.yourSystem}</span>}
        {assets?.checksum && (
          <p className="platform__meta">
            <a href={assets.checksum.browser_download_url}>{a.checksum}</a>
          </p>
        )}
        {assets && ANDROID_CERT_SHA256 && (
          <p className="platform__meta">
            {a.certificate}: <code>{ANDROID_CERT_SHA256}</code>
          </p>
        )}
        {state.status === "none" && <p className="platform__meta">{a.comingSoon}</p>}
      </div>
      {assets ? (
        <a className={`btn ${yours ? "btn--primary" : "btn--ghost"}`} href={assets.apk.browser_download_url}>
          <Icon name="download" />
          {a.downloadApk} ({version})
        </a>
      ) : (
        <a className="btn btn--ghost" href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
          {t.download.viewReleases}
          <Icon name="external" size={15} />
        </a>
      )}
    </div>
  );
}

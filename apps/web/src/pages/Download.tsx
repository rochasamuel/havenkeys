import { useEffect, useState } from "react";
import { AndroidDownload } from "../components/AndroidDownload";
import { Icon } from "../components/Icon";
import { Mark } from "../components/Mark";
import { useI18n } from "../i18n/context";
import { detectOs, type Os } from "../lib/os";
import {
  fetchLatestRelease,
  pickAsset,
  RELEASES_PAGE_URL,
  type LatestRelease,
  type Platform,
} from "../lib/releases";

const PLATFORMS: Array<{ id: Platform; os: Os }> = [
  { id: "windows", os: "windows" },
  { id: "macos", os: "macos" },
  { id: "linux-appimage", os: "linux" },
  { id: "linux-deb", os: "linux" },
];

type LoadState =
  | { status: "loading" }
  | { status: "ready"; release: LatestRelease }
  | { status: "unavailable" };

export function Download() {
  const { t } = useI18n();
  const d = t.download;
  const [state, setState] = useState<LoadState>({ status: "loading" });
  const [os] = useState(() => (typeof navigator === "undefined" ? null : detectOs(navigator.userAgent)));

  useEffect(() => {
    let cancelled = false;
    fetchLatestRelease().then((release) => {
      if (cancelled) return;
      setState(release ? { status: "ready", release } : { status: "unavailable" });
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="download">
      <section className="page-hero page-hero--download">
        <Mark size={64} />
        <h1>{d.title}</h1>
        <div aria-live="polite" className="download__status">
          {state.status === "ready" && (
            <p>
              {d.latest} <code>{state.release.tag_name}</code>
            </p>
          )}
          {state.status === "loading" && <p>{d.checking}</p>}
          {state.status === "unavailable" && (
            <p>
              {d.unavailable}{" "}
              <a href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
                {d.seeReleases}
              </a>
              .
            </p>
          )}
        </div>
      </section>

      <section className="download__grid" aria-label={d.installersAria}>
        {PLATFORMS.map((platform) => {
          const asset = state.status === "ready" ? pickAsset(state.release.assets, platform.id) : null;
          const yours = PLATFORMS.find((p) => p.os === os)?.id === platform.id;
          const copy = d.platforms[platform.id];
          return (
            <div className={`platform ${yours ? "platform--yours" : ""}`} key={platform.id}>
              <div className="platform__text">
                <h2>{copy.label}</h2>
                <p>{copy.format}</p>
                {yours && <span className="tag">{d.yourSystem}</span>}
              </div>
              {asset ? (
                <a className={`btn ${yours ? "btn--primary" : "btn--ghost"}`} href={asset.browser_download_url}>
                  <Icon name="download" />
                  {d.download}
                </a>
              ) : (
                <a className="btn btn--ghost" href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
                  {d.viewReleases}
                  <Icon name="external" size={15} />
                </a>
              )}
            </div>
          );
        })}
        <AndroidDownload yours={os === "android"} />
      </section>

      <section className="setup setup--android">
        <h2>{d.android.stepsTitle}</h2>
        <ol className="setup__steps">
          {d.android.steps.map((step) => (
            <li key={step}>
              <p>{step}</p>
            </li>
          ))}
        </ol>
        <p className="setup__note">{d.android.browsers}</p>
      </section>

      <section className="setup">
        <h2>{d.beforeTitle}</h2>
        <ol className="setup__steps">
          {d.steps.map((step) => (
            <li key={step.title}>
              <h3>{step.title}</h3>
              <div>
                <p>{step.body}</p>
                {step.actions && <div className="setup__actions">{step.actions}</div>}
              </div>
            </li>
          ))}
        </ol>
      </section>
    </div>
  );
}

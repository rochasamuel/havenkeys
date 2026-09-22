import { useEffect, useState } from "react";
import {
  fetchLatestRelease,
  pickAsset,
  RELEASES_PAGE_URL,
  type LatestRelease,
  type Platform,
} from "../lib/releases";

const PLATFORMS: Array<{ id: Platform; label: string; format: string }> = [
  { id: "windows", label: "Windows", format: ".msi installer" },
  { id: "macos", label: "macOS", format: ".dmg disk image" },
  { id: "linux-appimage", label: "Linux", format: ".AppImage" },
  { id: "linux-deb", label: "Linux (Debian/Ubuntu)", format: ".deb package" },
];

type LoadState =
  | { status: "loading" }
  | { status: "ready"; release: LatestRelease }
  | { status: "unavailable" };

export function Download() {
  const [state, setState] = useState<LoadState>({ status: "loading" });

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
    <section className="download">
      <h1>Download HavenKeys</h1>
      {state.status === "ready" && (
        <p className="download__status">Latest version: {state.release.tag_name}</p>
      )}
      {state.status === "loading" && (
        <p className="download__status">Checking latest release…</p>
      )}
      {state.status === "unavailable" && (
        <p className="download__status">
          Couldn't reach GitHub to find the latest build.{" "}
          <a href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
            View releases on GitHub
          </a>
          .
        </p>
      )}
      <div className="download__grid">
        {PLATFORMS.map((platform) => {
          const asset =
            state.status === "ready" ? pickAsset(state.release.assets, platform.id) : null;
          return (
            <div className="download__card" key={platform.id}>
              <h2>{platform.label}</h2>
              <p>{platform.format}</p>
              {asset ? (
                <a className="download__button" href={asset.browser_download_url}>
                  Download for {platform.label}
                </a>
              ) : (
                <a
                  className="download__button download__button--secondary"
                  href={RELEASES_PAGE_URL}
                  target="_blank"
                  rel="noreferrer"
                >
                  View releases
                </a>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

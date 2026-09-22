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
      <div className="download__notice">
        <h2>Before you install</h2>
        <ul>
          <li>
            HavenKeys needs an account on a <code>havenkeys-server</code>: run your own (see the{" "}
            <a
              href="https://github.com/rochasamuel/havenkeys/blob/main/docs/deployment.md"
              target="_blank"
              rel="noreferrer"
            >
              deployment guide
            </a>
            ) or get an invite from someone who does.
          </li>
          <li>
            The browser extension and its native messaging host are not in these installers yet.
            Build them from source with the steps in the{" "}
            <a href="https://github.com/rochasamuel/havenkeys#readme" target="_blank" rel="noreferrer">
              README
            </a>
            .
          </li>
          <li>
            The installers are unsigned, so Windows SmartScreen and macOS Gatekeeper will warn on
            first run. The Windows and macOS builds are new and less tested than Linux.
          </li>
        </ul>
      </div>
      <div aria-live="polite">
        {state.status === "ready" && (
          <p className="download__status">Latest version: {state.release.tag_name}</p>
        )}
        {state.status === "loading" && (
          <p className="download__status">Checking latest release…</p>
        )}
        {state.status === "unavailable" && (
          <p className="download__status">
            The latest release couldn't be loaded — there may not be one published yet.{" "}
            <a href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
              View releases on GitHub
            </a>
            .
          </p>
        )}
      </div>
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

export interface ReleaseAsset {
  name: string;
  browser_download_url: string;
}

export interface LatestRelease {
  tag_name: string;
  html_url: string;
  assets: ReleaseAsset[];
}

export type Platform = "windows" | "macos" | "linux-appimage" | "linux-deb";

// Order matters: the first matching suffix wins, so the preferred installer
// format for a platform (e.g. .msi over the NSIS .exe) is listed first.
const PLATFORM_SUFFIXES: Record<Platform, string[]> = {
  windows: [".msi", ".exe"],
  macos: [".dmg"],
  "linux-appimage": [".AppImage"],
  "linux-deb": [".deb"],
};

export const RELEASE_ASSET_PREFIX =
  "https://github.com/rochasamuel/havenkeys/releases/download/";

export function pickAsset(assets: ReleaseAsset[], platform: Platform): ReleaseAsset | null {
  const trustedAssets = assets.filter((asset) =>
    asset.browser_download_url.startsWith(RELEASE_ASSET_PREFIX),
  );
  for (const suffix of PLATFORM_SUFFIXES[platform]) {
    const match = trustedAssets.find((asset) => asset.name.endsWith(suffix));
    if (match) return match;
  }
  return null;
}

const RELEASES_API_URL = "https://api.github.com/repos/rochasamuel/havenkeys/releases/latest";
export const RELEASES_PAGE_URL = "https://github.com/rochasamuel/havenkeys/releases";

export async function fetchLatestRelease(): Promise<LatestRelease | null> {
  try {
    const response = await fetch(RELEASES_API_URL, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!response.ok) return null;
    const data = (await response.json()) as LatestRelease;
    if (!Array.isArray(data.assets)) return null;
    return data;
  } catch {
    return null;
  }
}

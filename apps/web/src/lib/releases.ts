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

export interface ReleaseListing extends LatestRelease {
  draft: boolean;
  prerelease: boolean;
}

/** SHA-256 of the Android release key's certificate; null until the key exists. */
export const ANDROID_CERT_SHA256: string | null = null;

const ANDROID_TAG_PREFIX = "android-v";
const RELEASES_LIST_URL = "https://api.github.com/repos/rochasamuel/havenkeys/releases?per_page=100";

/** The newest published Android release (GitHub lists newest first). */
export function pickAndroidRelease(releases: ReleaseListing[]): LatestRelease | null {
  return (
    releases.find((r) => !r.draft && !r.prerelease && r.tag_name.startsWith(ANDROID_TAG_PREFIX)) ?? null
  );
}

export function pickAndroidAssets(
  assets: ReleaseAsset[],
): { apk: ReleaseAsset; checksum: ReleaseAsset | null } | null {
  const trusted = assets.filter((a) => a.browser_download_url.startsWith(RELEASE_ASSET_PREFIX));
  const apk = trusted.find((a) => a.name.endsWith(".apk"));
  if (!apk) return null;
  const checksum = trusted.find((a) => a.name === `${apk.name}.sha256`) ?? null;
  return { apk, checksum };
}

export async function fetchLatestAndroidRelease(): Promise<LatestRelease | null> {
  try {
    const response = await fetch(RELEASES_LIST_URL, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!response.ok) return null;
    const data = (await response.json()) as unknown;
    if (!Array.isArray(data)) return null;
    return pickAndroidRelease(data as ReleaseListing[]);
  } catch {
    return null;
  }
}

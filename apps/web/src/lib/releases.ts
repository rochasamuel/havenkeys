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
export const ANDROID_CERT_SHA256: string | null =
  "02:AA:D9:EF:F6:CE:B3:6C:A6:34:7D:31:55:FB:78:A4:27:FB:6C:F3:68:ED:43:93:22:6D:D9:D4:FB:27:5D:D5";

const ANDROID_TAG_PREFIX = "android-v";
const RELEASES_LIST_URL = "https://api.github.com/repos/rochasamuel/havenkeys/releases?per_page=100";

/** Validates an asset: must have string name and browser_download_url. */
function isValidReleaseAsset(asset: unknown): asset is ReleaseAsset {
  return (
    typeof asset === "object" &&
    asset !== null &&
    typeof (asset as Record<string, unknown>).name === "string" &&
    typeof (asset as Record<string, unknown>).browser_download_url === "string"
  );
}

/** Validates a release: tag_name, draft, prerelease, html_url (must be on GitHub), and assets must be an array of valid items. */
function validateAndroidRelease(item: unknown): ReleaseListing | null {
  if (typeof item !== "object" || item === null) return null;
  const r = item as Record<string, unknown>;
  if (typeof r.tag_name !== "string") return null;
  if (typeof r.draft !== "boolean" || typeof r.prerelease !== "boolean") return null;
  if (typeof r.html_url !== "string") return null;
  if (!r.html_url.startsWith(RELEASES_PAGE_URL + "/")) return null;
  if (!Array.isArray(r.assets)) return null;

  // Filter assets to only valid ones (drop malformed items).
  const validAssets = (r.assets as unknown[]).filter(isValidReleaseAsset);

  return {
    tag_name: r.tag_name,
    html_url: r.html_url,
    draft: r.draft,
    prerelease: r.prerelease,
    assets: validAssets,
  };
}

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
    // Validate each release; skip invalid ones.
    const validReleases = data
      .map((item) => validateAndroidRelease(item))
      .filter((r): r is ReleaseListing => r !== null);
    return pickAndroidRelease(validReleases);
  } catch {
    return null;
  }
}

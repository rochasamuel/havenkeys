import { afterEach, describe, expect, it, vi } from "vitest";
import {
  fetchLatestAndroidRelease,
  fetchLatestRelease,
  pickAndroidAssets,
  pickAndroidRelease,
  pickAsset,
  RELEASE_ASSET_PREFIX,
  type ReleaseListing,
  type ReleaseAsset,
} from "./releases";

describe("pickAsset", () => {
  const assets: ReleaseAsset[] = [
    {
      name: "HavenKeys_0.1.0_x64-setup.exe",
      browser_download_url: `${RELEASE_ASSET_PREFIX}desktop-v0.1.0/HavenKeys_0.1.0_x64-setup.exe`,
    },
    {
      name: "HavenKeys_0.1.0_x64_en-US.msi",
      browser_download_url: `${RELEASE_ASSET_PREFIX}desktop-v0.1.0/HavenKeys_0.1.0_x64_en-US.msi`,
    },
    {
      name: "HavenKeys_0.1.0_aarch64.dmg",
      browser_download_url: `${RELEASE_ASSET_PREFIX}desktop-v0.1.0/HavenKeys_0.1.0_aarch64.dmg`,
    },
    {
      name: "HavenKeys_0.1.0_amd64.AppImage",
      browser_download_url: `${RELEASE_ASSET_PREFIX}desktop-v0.1.0/HavenKeys_0.1.0_amd64.AppImage`,
    },
    {
      name: "HavenKeys_0.1.0_amd64.deb",
      browser_download_url: `${RELEASE_ASSET_PREFIX}desktop-v0.1.0/HavenKeys_0.1.0_amd64.deb`,
    },
  ];

  it("prefers the .msi over the .exe for Windows", () => {
    expect(pickAsset(assets, "windows")?.name).toBe("HavenKeys_0.1.0_x64_en-US.msi");
  });

  it("finds the macOS .dmg", () => {
    expect(pickAsset(assets, "macos")?.name).toBe("HavenKeys_0.1.0_aarch64.dmg");
  });

  it("finds the Linux AppImage", () => {
    expect(pickAsset(assets, "linux-appimage")?.name).toBe("HavenKeys_0.1.0_amd64.AppImage");
  });

  it("finds the Linux .deb package", () => {
    expect(pickAsset(assets, "linux-deb")?.name).toBe("HavenKeys_0.1.0_amd64.deb");
  });

  it("returns null when no asset matches the platform", () => {
    expect(pickAsset([], "windows")).toBeNull();
  });

  it("ignores assets hosted anywhere else", () => {
    const spoofed: ReleaseAsset[] = [
      { name: "HavenKeys_0.1.0_x64_en-US.msi", browser_download_url: "https://evil.example/x.msi" },
    ];
    expect(pickAsset(spoofed, "windows")).toBeNull();
  });
});

describe("fetchLatestRelease", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns the parsed release on a successful response", async () => {
    const release = { tag_name: "desktop-v0.1.0", html_url: "https://x", assets: [] };
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: async () => release }),
    );
    expect(await fetchLatestRelease()).toEqual(release);
  });

  it("returns null on a non-ok response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false }));
    expect(await fetchLatestRelease()).toBeNull();
  });

  it("returns null when the response has no assets array", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: async () => ({ tag_name: "x" }) }),
    );
    expect(await fetchLatestRelease()).toBeNull();
  });

  it("returns null when the fetch throws", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    expect(await fetchLatestRelease()).toBeNull();
  });
});

describe("pickAndroidRelease", () => {
  const release = (tag: string, extra: Partial<ReleaseListing> = {}): ReleaseListing => ({
    tag_name: tag,
    html_url: `https://github.com/rochasamuel/havenkeys/releases/tag/${tag}`,
    assets: [],
    draft: false,
    prerelease: false,
    ...extra,
  });

  it("finds the newest Android release behind newer desktop releases", () => {
    const list = [release("desktop-v0.14.0"), release("android-v0.2.0"), release("android-v0.1.0")];
    expect(pickAndroidRelease(list)?.tag_name).toBe("android-v0.2.0");
  });

  it("skips drafts and pre-releases", () => {
    const list = [
      release("android-v0.3.0", { draft: true }),
      release("android-v0.2.0", { prerelease: true }),
      release("android-v0.1.0"),
    ];
    expect(pickAndroidRelease(list)?.tag_name).toBe("android-v0.1.0");
  });

  it("returns null without an Android release", () => {
    expect(pickAndroidRelease([release("desktop-v0.13.0")])).toBeNull();
  });
});

describe("pickAndroidAssets", () => {
  const asset = (name: string, url = `${RELEASE_ASSET_PREFIX}android-v0.1.0/${name}`): ReleaseAsset => ({
    name,
    browser_download_url: url,
  });

  it("finds the APK and its checksum", () => {
    const picked = pickAndroidAssets([asset("HavenKeys-0.1.0.apk.sha256"), asset("HavenKeys-0.1.0.apk")]);
    expect(picked?.apk.name).toBe("HavenKeys-0.1.0.apk");
    expect(picked?.checksum?.name).toBe("HavenKeys-0.1.0.apk.sha256");
  });

  it("ignores an APK hosted anywhere else", () => {
    expect(pickAndroidAssets([asset("HavenKeys-0.1.0.apk", "https://evil.example/HavenKeys-0.1.0.apk")])).toBeNull();
  });

  it("returns null when the release has no APK", () => {
    expect(pickAndroidAssets([asset("HavenKeys-0.1.0.apk.sha256")])).toBeNull();
  });
});

describe("fetchLatestAndroidRelease", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns the newest published Android release", async () => {
    const list = [
      { tag_name: "desktop-v0.13.0", html_url: "x", assets: [], draft: false, prerelease: false },
      { tag_name: "android-v0.1.0", html_url: "y", assets: [], draft: false, prerelease: false },
    ];
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => list }));
    expect((await fetchLatestAndroidRelease())?.tag_name).toBe("android-v0.1.0");
  });

  it("returns null when the request fails", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    expect(await fetchLatestAndroidRelease()).toBeNull();
  });

  it("returns null on a rate-limit response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false, json: async () => ({}) }));
    expect(await fetchLatestAndroidRelease()).toBeNull();
  });

  it("returns null when the answer is not a list", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => ({ message: "x" }) }));
    expect(await fetchLatestAndroidRelease()).toBeNull();
  });
});

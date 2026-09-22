import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchLatestRelease, pickAsset, type ReleaseAsset } from "./releases";

describe("pickAsset", () => {
  const assets: ReleaseAsset[] = [
    { name: "HavenKeys_0.1.0_x64-setup.exe", browser_download_url: "https://example.com/exe" },
    { name: "HavenKeys_0.1.0_x64_en-US.msi", browser_download_url: "https://example.com/msi" },
    { name: "HavenKeys_0.1.0_aarch64.dmg", browser_download_url: "https://example.com/dmg" },
    {
      name: "HavenKeys_0.1.0_amd64.AppImage",
      browser_download_url: "https://example.com/appimage",
    },
    { name: "HavenKeys_0.1.0_amd64.deb", browser_download_url: "https://example.com/deb" },
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

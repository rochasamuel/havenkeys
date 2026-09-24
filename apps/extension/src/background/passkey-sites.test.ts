import { describe, expect, it } from "vitest";
import raw from "../data/passkey-sites.json";
import { findPasskeySite, parsePasskeySites, PASSKEY_SITES, type PasskeySite } from "./passkey-sites";

const site = (name: string, domains: string[], help: string | null = `https://${domains[0]}/help`): PasskeySite => ({ name, domains, passwordless: true, mfa: false, help });
const SITES = [site("GitHub", ["github.com"]), site("Nintendo", ["nintendo.com"]), site("Nintendo Account", ["accounts.nintendo.com"]), site("Microsoft", ["microsoft.com", "live.com"])];

describe("committed directory", () => {
  it("validates and is what the extension loads", () => {
    const parsed = parsePasskeySites(raw);
    expect(parsed).not.toBeNull();
    expect(parsed!.length).toBeGreaterThan(50);
    expect(PASSKEY_SITES).toEqual(parsed);
    expect(parsed!.every((s) => s.help === null || s.help.startsWith("https://"))).toBe(true);
  });
});

describe("parsePasskeySites", () => {
  it("rejects anything off-shape", () => {
    const good = { name: "A", domains: ["a.com"], passwordless: true, mfa: false, help: null };
    expect(parsePasskeySites([good])).toEqual([good]);
    for (const bad of [
      { ...good, extra: 1 },
      { ...good, help: "http://a.com/" },
      { ...good, help: "javascript:alert(1)" },
      { ...good, domains: [] },
      { ...good, domains: ["A.com"] },
      { ...good, domains: ["a.com."] },
      { ...good, domains: ["localhost"] },
      { ...good, name: "" },
      { ...good, name: "a\u0007" },
      { ...good, mfa: "allowed" },
    ]) {
      expect(parsePasskeySites([bad])).toBeNull();
    }
    expect(parsePasskeySites({})).toBeNull();
  });
});

describe("findPasskeySite", () => {
  it("matches the host or a subdomain, never a look-alike", () => {
    expect(findPasskeySite("https://github.com/login", SITES)?.name).toBe("GitHub");
    expect(findPasskeySite("https://gist.github.com/", SITES)?.name).toBe("GitHub");
    expect(findPasskeySite("https://GitHub.com./login", SITES)?.name).toBe("GitHub");
    expect(findPasskeySite("https://login.live.com/", SITES)?.name).toBe("Microsoft");
    for (const u of ["https://github.com.evil.com/", "https://evilgithub.com/", "https://github.co/", "not a url", "https://127.0.0.1/"]) {
      expect(findPasskeySite(u, SITES)).toBeNull();
    }
  });

  it("prefers the longest matching domain", () => {
    expect(findPasskeySite("https://accounts.nintendo.com/login", SITES)?.name).toBe("Nintendo Account");
    expect(findPasskeySite("https://www.nintendo.com/", SITES)?.name).toBe("Nintendo");
  });
});

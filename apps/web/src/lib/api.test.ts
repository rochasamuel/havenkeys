import { describe, expect, it } from "vitest";
import { API_BASE, startSignup, verifySignup } from "./api";

type Call = { url: string; init: RequestInit };

function fakeFetch(status: number, body: unknown, calls: Call[] = []): typeof fetch {
  return (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(url), init: init ?? {} });
    return new Response(body === undefined ? null : JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json" },
    });
  }) as typeof fetch;
}

describe("signup api", () => {
  it("posts start to the fixed base URL with exactly the three fields", async () => {
    const calls: Call[] = [];
    const result = await startSignup(
      { email: "a@example.com", locale: "pt-BR", acceptedTerms: "2026-10-20" },
      fakeFetch(202, {}, calls),
    );
    expect(result).toEqual({ ok: true });
    expect(calls[0]?.url).toBe(`${API_BASE}/v1/signup/start`);
    expect(calls[0]?.init.method).toBe("POST");
    expect(JSON.parse(String(calls[0]?.init.body))).toEqual({
      email: "a@example.com",
      locale: "pt-BR",
      acceptedTerms: "2026-10-20",
    });
    expect(calls[0]?.init.credentials).toBe("omit");
  });

  it("returns the invite from verify and rejects a malformed one", async () => {
    const ok = await verifySignup({ email: "a@example.com", code: "123456" }, fakeFetch(200, { invite: "HKINV1-abcdefghijklmnop0123456789" }));
    expect(ok).toEqual({ ok: true, invite: "HKINV1-abcdefghijklmnop0123456789" });
    const bad = await verifySignup({ email: "a@example.com", code: "123456" }, fakeFetch(200, { invite: "nope" }));
    expect(bad).toEqual({ ok: false, failure: { kind: "network" } });
    const missing = await verifySignup({ email: "a@example.com", code: "123456" }, fakeFetch(200, {}));
    expect(missing.ok).toBe(false);
  });

  it("classifies failures by status", async () => {
    const cases: Array<[number, string]> = [
      [400, "invalid"],
      [429, "rate_limited"],
      [503, "unavailable"],
      [404, "closed"],
      [500, "network"],
    ];
    for (const [status, kind] of cases) {
      const r = await startSignup(
        { email: "a@example.com", locale: "en", acceptedTerms: "2026-10-20" },
        fakeFetch(status, { error: { code: "x", message: "y" } }),
      );
      expect(r).toEqual({ ok: false, failure: { kind } });
    }
    const thrown = (async () => {
      throw new TypeError("offline");
    }) as unknown as typeof fetch;
    const r = await startSignup({ email: "a@example.com", locale: "en", acceptedTerms: "2026-10-20" }, thrown);
    expect(r).toEqual({ ok: false, failure: { kind: "network" } });
  });
});

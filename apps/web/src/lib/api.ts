// The only API the site calls. The CSP in vercel.json allows exactly this
// host for connect-src; a second host needs both changed.
export const API_BASE = "https://api.havenkeys.net";

export type ApiFailure = {
  kind: "invalid" | "rate_limited" | "unavailable" | "closed" | "network";
};

export type Locale = "en" | "pt-BR";

type Outcome<T> = ({ ok: true } & T) | { ok: false; failure: ApiFailure };

function classify(status: number): ApiFailure["kind"] {
  if (status === 400) return "invalid";
  if (status === 429) return "rate_limited";
  if (status === 503) return "unavailable";
  if (status === 404) return "closed";
  return "network";
}

async function post(path: string, body: unknown, fetchFn: typeof fetch): Promise<Response | null> {
  try {
    return await fetchFn(`${API_BASE}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify(body),
      credentials: "omit",
      referrerPolicy: "no-referrer",
    });
  } catch {
    return null;
  }
}

export async function startSignup(
  input: { email: string; locale: Locale; acceptedTerms: string },
  fetchFn: typeof fetch = fetch,
): Promise<Outcome<Record<never, never>>> {
  const res = await post("/v1/signup/start", input, fetchFn);
  if (!res) return { ok: false, failure: { kind: "network" } };
  if (res.status === 202) return { ok: true };
  return { ok: false, failure: { kind: classify(res.status) } };
}

export async function verifySignup(
  input: { email: string; code: string },
  fetchFn: typeof fetch = fetch,
): Promise<Outcome<{ invite: string }>> {
  const res = await post("/v1/signup/verify", input, fetchFn);
  if (!res) return { ok: false, failure: { kind: "network" } };
  if (res.status !== 200) return { ok: false, failure: { kind: classify(res.status) } };
  let data: unknown;
  try {
    data = await res.json();
  } catch {
    return { ok: false, failure: { kind: "network" } };
  }
  const invite = (data as { invite?: unknown })?.invite;
  // The app accepts only this shape; anything else is not an invite.
  if (typeof invite !== "string" || !/^HKINV1-[A-Za-z0-9_-]{16,2048}$/.test(invite)) {
    return { ok: false, failure: { kind: "network" } };
  }
  return { ok: true, invite };
}

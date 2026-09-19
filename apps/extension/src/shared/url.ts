import { MAX_URL_BYTES } from "@havenkeys/protocol";

/**
 * The page URL sent to the desktop for matching.
 *
 * Only http(s) pages qualify. Credentials, query and fragment are removed
 * before the URL leaves the browser: matching ignores them, and they often
 * carry tokens. The desktop re-parses and re-checks the URL regardless.
 *
 * The input must come from the browser (tab or frame URL), never from page
 * content.
 */
export function pageUrlForRequest(raw: string | undefined): string | null {
  if (!raw) return null;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return null;
  url.username = "";
  url.password = "";
  url.search = "";
  url.hash = "";
  const out = url.toString();
  return new TextEncoder().encode(out).length <= MAX_URL_BYTES ? out : null;
}

/** Host name for display, or null. */
export function displayHost(pageUrl: string | null): string | null {
  if (!pageUrl) return null;
  try {
    return new URL(pageUrl).hostname;
  } catch {
    return null;
  }
}

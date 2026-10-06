// Addresses the site links to, in one place.
export const GITHUB = "https://github.com/rochasamuel/havenkeys";
export const CHROME_STORE = "https://chromewebstore.google.com/detail/havenkeys/fmmfkakdkkcfpdnfmbngnlelbfaogafo";
export const FIREFOX_STORE = "https://addons.mozilla.org/firefox/addon/havenkeys/";
export const INVITE_EMAIL = "invite@havenkeys.net";
/** Set once the owner publishes the Railway template (deploy/railway/README.md). */
export const RAILWAY_TEMPLATE_URL = "";

export function inviteHref(subject: string): string {
  return `mailto:${INVITE_EMAIL}?subject=${encodeURIComponent(subject)}`;
}

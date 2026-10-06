// "Sign in with" display helpers. The desktop only shows these; which pages
// a login may be used on is decided in Rust.

import type { SecretEdit } from "./secretEdit";
import type { SignInWith, SsoProvider } from "./types";

/** Alphabetical by name, as havenkeys-core's `SsoProvider::ALL`. */
export const PROVIDER_ORDER: SsoProvider[] = ["apple", "discord", "facebook", "github", "gitlab", "google", "linkedin", "microsoft", "x"];
export const PROVIDER_NAMES: Record<SsoProvider, string> = {
  apple: "Apple",
  discord: "Discord",
  facebook: "Facebook",
  github: "GitHub",
  gitlab: "GitLab",
  google: "Google",
  linkedin: "LinkedIn",
  microsoft: "Microsoft",
  x: "X",
};

export function ssoSubtitle(s: SignInWith): string {
  return s.account ? `${PROVIDER_NAMES[s.provider]} · ${s.account}` : PROVIDER_NAMES[s.provider];
}

/**
 * Whether picking a provider should tuck the username/password rows away
 * under "Also has a password". Only true for an untouched, empty password
 * on an item that never had one: typed input or an existing password are
 * never collapsed out from under the user.
 */
export function shouldCollapse(username: string, password: SecretEdit, hasExistingPassword: boolean): boolean {
  if (hasExistingPassword) return false;
  if (username.trim() !== "") return false;
  return password.mode === "set" && password.value === "";
}

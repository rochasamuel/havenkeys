/*
 * Every word the extension shows, in English. pt-BR.ts must match this
 * shape exactly (it is typed as Messages), so a string added here and
 * forgotten there fails the typecheck instead of shipping half-translated.
 *
 * Product names (HavenKeys, Secret Key, Emergency Kit, havenkeys-server)
 * stay in English in every locale. Strings are inserted with textContent
 * only; parameterised strings are functions.
 *
 * The manifest's name and description live in manifest/_locales instead,
 * because the browser reads them before any script runs.
 */
export const en = {};

export type Messages = typeof en;

/*
 * Every word the desktop app shows, in English. pt-BR.ts must match this
 * shape exactly (it is typed as Messages), so a string added here and
 * forgotten there fails the typecheck instead of shipping half-translated.
 *
 * Product names (HavenKeys, Secret Key, Emergency Kit, havenkeys-server)
 * stay in English in every locale, as do URLs and keyboard keys. Strings
 * are rendered as React text only; parameterised strings are functions.
 */

export const en = {
  settings: {
    language: "Language",
    languageAuto: "Automatic",
  },
};

export type Messages = typeof en;

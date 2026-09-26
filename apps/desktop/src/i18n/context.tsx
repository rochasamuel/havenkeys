import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import { en, type Messages } from "./en";
import { ptBR } from "./pt-BR";
import { effectiveLocale, type Locale, type Preference } from "./locale";
import { readPreference, writePreference } from "./preference";

const MESSAGES: Record<Locale, Messages> = { en, "pt-BR": ptBR };

interface I18n {
  locale: Locale;
  t: Messages;
  /** What the user picked in Settings ("auto" follows the OS). */
  preference: Preference;
  setPreference: (p: Preference) => void;
}

const I18nContext = createContext<I18n>({
  locale: "en",
  t: en,
  preference: "auto",
  setPreference: () => undefined,
});

function osLanguages(): readonly string[] {
  if (navigator.languages?.length) return navigator.languages;
  return navigator.language ? [navigator.language] : [];
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [preference, setPreferenceState] = useState<Preference>(readPreference);
  const [languages, setLanguages] = useState<readonly string[]>(osLanguages);
  const locale = effectiveLocale(preference, languages);

  // The OS language can change while the app runs; "auto" follows it.
  useEffect(() => {
    const onChange = () => setLanguages(osLanguages());
    window.addEventListener("languagechange", onChange);
    return () => window.removeEventListener("languagechange", onChange);
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale;
    // The tray menu is built in Rust; relabel it. Failure leaves it English.
    api.setUiLanguage(locale).catch(() => undefined);
  }, [locale]);

  const setPreference = useCallback((p: Preference) => {
    writePreference(p);
    setPreferenceState(p);
  }, []);

  const value = useMemo(
    () => ({ locale, t: MESSAGES[locale], preference, setPreference }),
    [locale, preference, setPreference],
  );
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  return useContext(I18nContext);
}

import { createContext, useContext, useEffect, type ReactNode } from "react";
import { en, type Messages } from "./en";
import { ptBR } from "./pt-BR";
import { LOCALES, localizePath, type Locale } from "./locale";

const MESSAGES: Record<Locale, Messages> = { en, "pt-BR": ptBR };

interface I18n {
  locale: Locale;
  t: Messages;
  /** A locale-neutral path ("/download") in the current locale. */
  path: (p: string) => string;
}

const I18nContext = createContext<I18n>({ locale: "en", t: en, path: (p) => p });

export function I18nProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  const t = MESSAGES[locale];

  useEffect(() => {
    document.documentElement.lang = LOCALES[locale].htmlLang;
    document.title = t.meta.title;
    document.querySelector('meta[name="description"]')?.setAttribute("content", t.meta.description);
  }, [locale, t]);

  return (
    <I18nContext.Provider value={{ locale, t, path: (p) => localizePath(p, locale) }}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): I18n {
  return useContext(I18nContext);
}

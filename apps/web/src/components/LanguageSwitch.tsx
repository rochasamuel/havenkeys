import { Link, useLocation } from "react-router-dom";
import { useI18n } from "../i18n/context";
import { LOCALES, switchLocalePath, type Locale } from "../i18n/locale";
import { writePreference } from "../i18n/preference";

/** A compact toggle for the nav: shows the other language's code. */
export function LanguageToggle() {
  const { locale, t } = useI18n();
  const { pathname, hash } = useLocation();
  const other: Locale = locale === "en" ? "pt-BR" : "en";
  return (
    <Link
      className="lang-toggle"
      to={switchLocalePath(pathname, hash, other)}
      hrefLang={LOCALES[other].htmlLang}
      lang={LOCALES[other].htmlLang}
      aria-label={t.nav.switchAria}
      onClick={() => writePreference(other)}
    >
      {t.nav.switchShort}
    </Link>
  );
}

/** Both languages spelled out, for the footer. */
export function LanguageList() {
  const { locale, t } = useI18n();
  const { pathname, hash } = useLocation();
  return (
    <nav className="lang-list" aria-label={t.footer.languageAria}>
      {(Object.keys(LOCALES) as Locale[]).map((l) =>
        l === locale ? (
          <span key={l} aria-current="true" lang={LOCALES[l].htmlLang}>
            {LOCALES[l].label}
          </span>
        ) : (
          <Link
            key={l}
            to={switchLocalePath(pathname, hash, l)}
            hrefLang={LOCALES[l].htmlLang}
            lang={LOCALES[l].htmlLang}
            onClick={() => writePreference(l)}
          >
            {LOCALES[l].label}
          </Link>
        ),
      )}
    </nav>
  );
}

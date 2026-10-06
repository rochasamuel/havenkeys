import { Link } from "react-router-dom";
import { useI18n } from "../i18n/context";
import { LanguageList } from "./LanguageSwitch";
import { Mark } from "./Mark";

export function Footer() {
  const { t, path } = useI18n();
  return (
    <footer className="footer">
      <div className="footer__inner">
        <div className="footer__brand">
          <Mark size={30} />
          <div>
            <strong>HavenKeys</strong>
            <span>{t.footer.tagline}</span>
          </div>
        </div>
        <nav className="footer__links" aria-label={t.footer.aria}>
          <Link to={path("/download")}>{t.footer.download}</Link>
          <Link to={path("/security")}>{t.footer.security}</Link>
          <Link to={path("/self-host")}>{t.footer.selfHost}</Link>
          <Link to={path("/developers")}>{t.footer.developers}</Link>
          <a href="https://github.com/rochasamuel/havenkeys" target="_blank" rel="noreferrer">
            {t.footer.github}
          </a>
          <Link to={path("/privacy")}>{t.footer.privacy}</Link>
          <Link to={path("/delete-account")}>{t.footer.deleteAccount}</Link>
          <Link to={path("/terms")}>{t.footer.terms}</Link>
        </nav>
        <p className="footer__disclaimer">{t.common.disclaimer}</p>
        <div className="footer__base">
          <p className="footer__copy">
            © {new Date().getFullYear()} HavenKeys · {t.footer.license}
          </p>
          <LanguageList />
        </div>
      </div>
    </footer>
  );
}

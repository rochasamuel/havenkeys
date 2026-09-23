import { Link, NavLink } from "react-router-dom";
import { useI18n } from "../i18n/context";
import { Icon } from "./Icon";
import { LanguageToggle } from "./LanguageSwitch";
import { Mark } from "./Mark";

export function Nav() {
  const { t, path } = useI18n();
  const home = path("/");
  return (
    <header className="nav">
      <div className="nav__inner">
        <Link to={home} className="nav__brand" aria-label={t.nav.homeAria}>
          <Mark size={26} />
          <span>HavenKeys</span>
        </Link>
        <nav className="nav__links" aria-label={t.nav.mainAria}>
          <a href={`${home}#journey`} className="nav__hide-sm">
            {t.nav.howItWorks}
          </a>
          <a href={`${home}#browser`} className="nav__hide-sm">
            {t.nav.browser}
          </a>
          <NavLink to={path("/security")}>{t.nav.security}</NavLink>
          <a
            href="https://github.com/rochasamuel/havenkeys"
            target="_blank"
            rel="noreferrer"
            className="nav__icon"
            aria-label={t.nav.githubAria}
          >
            <Icon name="github" size={19} />
          </a>
          <LanguageToggle />
          <Link to={path("/download")} className="btn btn--primary btn--sm">
            {t.nav.download}
          </Link>
        </nav>
      </div>
    </header>
  );
}

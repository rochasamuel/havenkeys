import { Link, NavLink } from "react-router-dom";
import { useI18n } from "../i18n/context";
import { GITHUB } from "../lib/links";
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
          <NavLink to={path("/security")}>{t.nav.security}</NavLink>
          <NavLink to={path("/self-host")} className="nav__hide-sm">
            {t.nav.selfHost}
          </NavLink>
          <NavLink to={path("/developers")} className="nav__hide-sm">
            {t.nav.developers}
          </NavLink>
          <a
            href={GITHUB}
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

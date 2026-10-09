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
          <span className="tag tag--beta">{t.common.beta}</span>
        </Link>
        <nav className="nav__links" aria-label={t.nav.mainAria}>
          <NavLink to={path("/security")}>{t.nav.security}</NavLink>
          <NavLink to={path("/pricing")} className="nav__hide-sm">
            {t.nav.pricing}
          </NavLink>
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
          <NavLink to={path("/download")} className="nav__hide-sm">
            {t.nav.download}
          </NavLink>
          <Link to={path("/signup")} className="btn btn--primary btn--sm">
            {t.common.createAccount}
          </Link>
        </nav>
      </div>
    </header>
  );
}

import { Link } from "react-router-dom";
import popup from "../assets/shots/popup.png";
import menuLogins from "../assets/shots/menu-logins.png";
import desktopVault from "../assets/shots/desktop-vault.png";
import { Guilloche } from "../components/Guilloche";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";
import { CHROME_STORE, FIREFOX_STORE, GITHUB } from "../lib/links";

export function Home() {
  const { t, path } = useI18n();
  const h = t.home;
  return (
    <>
      <section className="hero">
        <Guilloche className="hero__rosette" />
        <div className="hero__copy">
          <h1>{h.heroTitle}</h1>
          <p className="hero__lede">{h.heroLede}</p>
          <div className="hero__actions">
            <Link to={path("/download")} className="btn btn--primary btn--lg">
              <Icon name="download" />
              {t.common.downloadCta}
            </Link>
            <Link to={path("/signup")} className="btn btn--ghost btn--lg">
              {t.common.createAccount}
            </Link>
          </div>
          <p className="hero__meta">{h.heroMeta}</p>
          <p className="hero__meta">{t.common.betaNotice}</p>
        </div>

        <div className="hero__stage">
          <figure className="window">
            <div className="window__bar" aria-hidden="true">
              <span />
              <span />
              <span />
              <em>HavenKeys</em>
            </div>
            <img className="window__shot" src={desktopVault} width={1040} height={660} alt={h.desktopAlt} />
          </figure>
          <img className="hero__popup" src={popup} width={320} height={251} alt={h.popupAlt} />
          <img className="hero__menu" src={menuLogins} width={340} height={136} alt={h.menuAlt} />
        </div>
      </section>

      <section className="section" id="browser">
        <div className="section__head">
          <h2>{h.cardsTitle}</h2>
          <p>{h.cardsLede}</p>
        </div>
        <ul className="cards">
          {h.cards.map((c) => (
            <li key={c.title} className="card">
              <Icon name={c.icon} size={22} />
              <h3>{c.title}</h3>
              <p>{c.text}</p>
            </li>
          ))}
        </ul>
      </section>

      <section className="section trust" id="journey">
        <div className="section__head">
          <h2>{h.trustTitle}</h2>
          <p>{h.trustBody}</p>
          <Link to={path("/developers")} className="text-link">
            {h.trustLink} <Icon name="arrowRight" size={15} />
          </Link>
        </div>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{h.stepsTitle}</h2>
        </div>
        <ol className="steps">
          {h.steps.map((s, i) => (
            <li key={s.title} className="step">
              <span className="step__n" aria-hidden="true">
                {i + 1}
              </span>
              <h3>{s.title}</h3>
              <p>{s.text}</p>
            </li>
          ))}
        </ol>
      </section>

      <section className="section selfhost-teaser">
        <div className="section__head">
          <h2>{h.selfHostTitle}</h2>
          <p>{h.selfHostBody}</p>
          <Link to={path("/self-host")} className="btn btn--ghost">
            <Icon name="server" size={16} />
            {h.selfHostCta}
          </Link>
        </div>
      </section>

      <section className="closer">
        <Guilloche className="closer__rosette" />
        <h2>{h.closerTitle}</h2>
        <p>{h.closerLede}</p>
        <div className="hero__actions">
          <Link to={path("/download")} className="btn btn--primary btn--lg">
            <Icon name="download" />
            {t.common.downloadCta}
          </Link>
          <Link to={path("/signup")} className="btn btn--ghost btn--lg">
            {t.common.createAccount}
          </Link>
          <a className="btn btn--ghost btn--lg" href={CHROME_STORE} target="_blank" rel="noopener noreferrer">
            <Icon name="external" size={15} />
            {t.common.addToChrome}
          </a>
          <a className="btn btn--ghost btn--lg" href={FIREFOX_STORE} target="_blank" rel="noopener noreferrer">
            <Icon name="external" size={15} />
            {t.common.addToFirefox}
          </a>
          <a href={GITHUB} className="btn btn--ghost btn--lg" target="_blank" rel="noreferrer">
            <Icon name="github" />
            {h.readSource}
          </a>
        </div>
      </section>
    </>
  );
}

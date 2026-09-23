import { Link } from "react-router-dom";
import popup from "../assets/shots/popup.png";
import menuLogins from "../assets/shots/menu-logins.png";
import desktopVault from "../assets/shots/desktop-vault.png";
import { BrowserShowcase } from "../components/browser/BrowserShowcase";
import { EmergencyKit } from "../components/EmergencyKit";
import { Guilloche } from "../components/Guilloche";
import { Icon } from "../components/Icon";
import { Journey } from "../components/journey/Journey";
import { useI18n } from "../i18n/context";

const GITHUB = "https://github.com/rochasamuel/havenkeys";

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
            <a href="#journey" className="btn btn--ghost btn--lg">
              {h.seeHow}
              <Icon name="arrowDown" />
            </a>
          </div>
          <p className="hero__meta">{h.heroMeta}</p>
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

      <section className="section intro" id="journey">
        <div className="section__head">
          <h2>{h.journeyTitle}</h2>
          <p>{h.journeyLede}</p>
        </div>
        <Journey />
      </section>

      <section className="section browser-section" id="browser">
        <div className="section__head">
          <h2>{h.browserTitle}</h2>
          <p>{h.browserLede}</p>
        </div>
        <BrowserShowcase />
      </section>

      <section className="section desktop-section">
        <div className="desktop-section__grid">
          <div className="section__head section__head--left">
            <h2>{h.desktopTitle}</h2>
            <p>{h.desktopLede}</p>
          </div>
          <dl className="features">
            {h.features.map((f) => (
              <div key={f.term}>
                <dt>{f.term}</dt>
                <dd>{f.text}</dd>
              </div>
            ))}
          </dl>
        </div>
      </section>

      <section className="paper">
        <div className="paper__inner">
          <div className="paper__copy">
            <h2>{h.paperTitle}</h2>
            <p>{h.paperP1}</p>
            <p>{h.paperP2}</p>
            <p className="paper__warn">
              <Icon name="lock" size={16} />
              {h.paperWarn}
            </p>
          </div>
          <EmergencyKit />
        </div>
      </section>

      <section className="section ledger-section">
        <div className="section__head">
          <h2>{h.ledgerTitle}</h2>
          <p>{h.ledgerLede}</p>
        </div>
        <div className="ledger">
          <div className="ledger__col ledger__col--yes">
            <h3>{h.defendsTitle}</h3>
            <ul>
              {h.defends.map((d) => (
                <li key={d}>
                  <Icon name="check" size={16} />
                  <span>{d}</span>
                </li>
              ))}
            </ul>
          </div>
          <div className="ledger__col ledger__col--no">
            <h3>{h.doesntTitle}</h3>
            <ul>
              {h.doesnt.map((d) => (
                <li key={d}>
                  <Icon name="cross" size={16} />
                  <span>{d}</span>
                </li>
              ))}
            </ul>
          </div>
        </div>
        <div className="ledger__foot">
          <p className="disclaimer">{t.common.disclaimer}</p>
          <Link to={path("/security")} className="btn btn--ghost">
            {h.securityOverview}
            <Icon name="arrowRight" />
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
          <a href={GITHUB} className="btn btn--ghost btn--lg" target="_blank" rel="noreferrer">
            <Icon name="github" />
            {h.readSource}
          </a>
        </div>
      </section>
    </>
  );
}

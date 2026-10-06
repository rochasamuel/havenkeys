import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";
import { GITHUB, inviteHref, RAILWAY_TEMPLATE_URL } from "../lib/links";

const GUIDE = `${GITHUB}/blob/main/docs/self-hosting.md`;

export function SelfHost() {
  const { t } = useI18n();
  const s = t.selfHost;
  return (
    <div className="selfhost">
      <section className="page-hero">
        <h1>{s.title}</h1>
        <p className="page-hero__lede">{s.lede}</p>
      </section>

      <section className="section">
        <div className="section__head"><h2>{s.needTitle}</h2></div>
        <ul className="plain-list plain-list--ok">
          {s.needs.map((n) => (
            <li key={n}><Icon name="check" size={16} /><span>{n}</span></li>
          ))}
        </ul>
      </section>

      <section className="section">
        <div className="section__head"><h2>{s.stepsTitle}</h2></div>
        <ol className="selfhost__steps">
          {s.steps.map((step, i) => (
            <li key={step.title}>
              <span className="step__n" aria-hidden="true">{i + 1}</span>
              <div>
                <h3>{step.title}</h3>
                <p>{step.text}</p>
                {step.code && <pre className="code"><code>{step.code}</code></pre>}
              </div>
            </li>
          ))}
        </ol>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{s.railwayTitle}</h2>
          <p>{s.railwayBody}</p>
          {RAILWAY_TEMPLATE_URL ? (
            <a className="btn btn--primary" href={RAILWAY_TEMPLATE_URL} target="_blank" rel="noopener noreferrer">
              {s.railwayCta} <Icon name="external" size={15} />
            </a>
          ) : (
            <p className="muted">{s.railwaySoon}</p>
          )}
        </div>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{s.backupsTitle}</h2>
          <p>{s.backupsBody}</p>
          <h2>{s.chargeTitle}</h2>
          <p>{s.chargeBody}</p>
          <div className="hero__actions">
            <a className="btn btn--ghost" href={GUIDE} target="_blank" rel="noreferrer">
              {s.guideCta} <Icon name="external" size={15} />
            </a>
            <a className="btn btn--ghost" href={inviteHref(t.common.inviteSubject)}>{s.inviteInstead}</a>
          </div>
        </div>
      </section>
    </div>
  );
}

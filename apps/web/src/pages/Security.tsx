import { Link } from "react-router-dom";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";

export function Security() {
  const { t, path } = useI18n();
  const s = t.security;
  return (
    <div className="security-plain">
      <section className="page-hero">
        <h1>{s.heroTitle}</h1>
        <p className="page-hero__lede">{s.heroLede}</p>
      </section>
      <section className="section">
        <div className="section__head">
          <h2>{s.protectsTitle}</h2>
        </div>
        <ul className="cards">
          {s.protects.map((p) => (
            <li key={p.title} className="card">
              <Icon name="check" size={20} />
              <h3>{p.title}</h3>
              <p>{p.text}</p>
            </li>
          ))}
        </ul>
      </section>
      <section className="section">
        <div className="section__head">
          <h2>{s.limitsTitle}</h2>
        </div>
        <ul className="plain-list">
          {s.limits.map((l) => (
            <li key={l}>
              <Icon name="cross" size={16} />
              <span>{l}</span>
            </li>
          ))}
        </ul>
      </section>
      <section className="section">
        <div className="section__head">
          <h2>{s.auditTitle}</h2>
          <p>{s.auditBody}</p>
          <p className="disclaimer">{t.common.disclaimer}</p>
        </div>
      </section>
      <section className="section">
        <div className="section__head">
          <h2>{s.deepTitle}</h2>
          <p>{s.deepBody}</p>
          <Link to={path("/developers")} className="btn btn--ghost">
            {s.deepCta} <Icon name="arrowRight" />
          </Link>
        </div>
      </section>
    </div>
  );
}

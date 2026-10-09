import { Link } from "react-router-dom";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";

export function Pricing() {
  const { t, path } = useI18n();
  const p = t.pricing;
  return (
    <section className="pricing">
      <div className="section__head">
        <h1>{p.title}</h1>
        <p>{p.lede}</p>
      </div>
      <div className="pricing__card">
        <h2>{p.plan}</h2>
        <p className="pricing__price">{p.priceSoon}</p>
        <p className="pricing__trial">{p.trial}</p>
        <ul className="plain-list plain-list--ok">
          {p.includes.map((line) => (
            <li key={line}>
              <Icon name="check" size={16} />
              {line}
            </li>
          ))}
        </ul>
        <Link to={path("/signup")} className="btn btn--primary btn--lg">
          {p.cta}
        </Link>
        <p className="pricing__note">{p.subscribeSoon}</p>
        <p className="pricing__note">{t.common.betaNotice}</p>
      </div>
      <h2 className="pricing__after">{p.afterTitle}</h2>
      <ul className="plain-list plain-list--ok">
        {p.afterTrial.map((line) => (
          <li key={line}>
            <Icon name="check" size={16} />
            {line}
          </li>
        ))}
      </ul>
      <p>
        <Link to={path("/self-host")} className="text-link">
          {p.selfHost}
        </Link>
      </p>
    </section>
  );
}

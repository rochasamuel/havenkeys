import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";

const DOC = "https://github.com/rochasamuel/havenkeys/blob/main/docs/";

export function Security() {
  const { t } = useI18n();
  const s = t.security;
  return (
    <div className="security">
      <section className="page-hero">
        <h1>{s.heroTitle}</h1>
        <p className="page-hero__lede">{s.heroLede}</p>
        <p className="disclaimer">{t.common.disclaimer}</p>
      </section>

      <section className="section split">
        <div className="section__head section__head--left">
          <h2>{s.chainTitle}</h2>
          <p>{s.chainLede}</p>
          <a className="text-link" href={`${DOC}crypto.md`} target="_blank" rel="noreferrer">
            {s.chainLink} <Icon name="external" size={15} />
          </a>
        </div>
        <ol className="chain">
          {s.hierarchy.map((k) => (
            <li key={k.name} className={`chain__node chain__node--${k.tone}`}>
              <strong>{k.name}</strong>
              <span>{k.detail}</span>
            </li>
          ))}
        </ol>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{s.zonesTitle}</h2>
          <p>{s.zonesLede}</p>
        </div>
        <div className="zones">
          {s.zones.map((z, i) => (
            <div key={z.name} className={`zone ${i === 0 ? "zone--core" : ""}`}>
              <h3>{z.name}</h3>
              <p className="zone__where">{z.where}</p>
              <dl>
                <dt>{s.zoneGets}</dt>
                <dd>{z.holds}</dd>
                <dt>{s.zoneLimits}</dt>
                <dd>{z.limit}</dd>
              </dl>
            </div>
          ))}
        </div>
      </section>

      <section className="section split">
        <div className="section__head section__head--left">
          <h2>{s.permTitle}</h2>
          <p>{s.permLede}</p>
          <p className="muted">
            {s.notRequested} <code>&lt;all_urls&gt;</code>, <code>tabs</code>, <code>storage</code>,{" "}
            <code>cookies</code>, <code>webRequest</code>, <code>clipboardWrite</code>,{" "}
            <code>notifications</code>.
          </p>
        </div>
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th scope="col">{s.permHead[0]}</th>
                <th scope="col">{s.permHead[1]}</th>
              </tr>
            </thead>
            <tbody>
              {s.permissions.map((p) => (
                <tr key={p.name}>
                  <th scope="row">
                    <code>{p.name}</code>
                    <span className={p.optional ? "tag tag--opt" : "tag"}>
                      {p.optional ? s.optionalTag : s.always}
                    </span>
                  </th>
                  <td>{p.why}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{s.attacksTitle}</h2>
          <p>{s.attacksLede}</p>
        </div>
        <div className="table-wrap">
          <table className="table table--attacks">
            <thead>
              <tr>
                <th scope="col">{s.attacksHead[0]}</th>
                <th scope="col">{s.attacksHead[1]}</th>
              </tr>
            </thead>
            <tbody>
              {s.attacks.map(([attempt, result]) => (
                <tr key={attempt}>
                  <td>{attempt}</td>
                  <td>
                    <span className="result">
                      <Icon name="lock" size={14} />
                      {result}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="section split">
        <div className="section__head section__head--left">
          <h2>{s.scopeTitle}</h2>
          <p>{s.scopeLede}</p>
        </div>
        <ul className="plain-list">
          {s.outOfScope.map((o) => (
            <li key={o}>
              <Icon name="cross" size={16} />
              <span>{o}</span>
            </li>
          ))}
        </ul>
      </section>

      <section className="section">
        <div className="section__head">
          <h2>{s.readingTitle}</h2>
          <p>{s.readingLede}</p>
        </div>
        <ul className="reading">
          {s.reading.map((d) => (
            <li key={d.file}>
              <a href={`${DOC}${d.file}`} target="_blank" rel="noreferrer">
                <strong>{d.title}</strong>
                <span>{d.body}</span>
                <code>docs/{d.file}</code>
                <Icon name="external" size={16} className="reading__icon" />
              </a>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

import { useI18n } from "../i18n/context";

export function Terms() {
  const { t } = useI18n();
  return (
    <section className="docs-page">
      <h1>{t.terms.title}</h1>
      <p className="docs-page__updated">{t.terms.updated}</p>
      {t.terms.body}
    </section>
  );
}

import { useI18n } from "../i18n/context";

export function Privacy() {
  const { t } = useI18n();
  return (
    <section className="docs-page">
      <h1>{t.privacy.title}</h1>
      <p className="docs-page__updated">{t.privacy.updated}</p>
      {t.privacy.body}
    </section>
  );
}

import { useI18n } from "../i18n/context";

export function DeleteAccount() {
  const { t } = useI18n();
  return (
    <section className="docs-page">
      <h1>{t.deleteAccount.title}</h1>
      <p className="docs-page__updated">{t.deleteAccount.updated}</p>
      {t.deleteAccount.body}
    </section>
  );
}

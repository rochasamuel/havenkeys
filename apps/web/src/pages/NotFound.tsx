import { useI18n } from "../i18n/context";

export function NotFound() {
  const { t, path } = useI18n();
  return (
    <section className="docs-page">
      <h1>{t.notFound.title}</h1>
      <p>{t.notFound.body(path("/"))}</p>
    </section>
  );
}

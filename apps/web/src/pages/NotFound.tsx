import { Link } from "react-router-dom";

export function NotFound() {
  return (
    <section className="docs-page">
      <h1>Page not found</h1>
      <p>
        There's nothing at this address. <Link to="/">Go to the homepage</Link>.
      </p>
    </section>
  );
}

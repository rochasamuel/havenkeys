import { Link } from "react-router-dom";

export function Nav() {
  return (
    <header className="nav">
      <div className="nav__inner">
        <Link to="/" className="nav__brand">
          HavenKeys
        </Link>
        <nav className="nav__links">
          <Link to="/security">Security</Link>
          <a href="https://github.com/rochasamuel/havenkeys" target="_blank" rel="noreferrer">
            GitHub
          </a>
          <Link to="/download" className="nav__cta">
            Download
          </Link>
        </nav>
      </div>
    </header>
  );
}

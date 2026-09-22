import { Link } from "react-router-dom";

export function Footer() {
  return (
    <footer className="footer">
      <div className="footer__inner">
        <p className="footer__disclaimer">
          This software has not undergone an independent security audit and should not be
          considered a replacement for professionally audited password managers for high-value
          production use.
        </p>
        <div className="footer__links">
          <a href="https://github.com/rochasamuel/havenkeys" target="_blank" rel="noreferrer">
            GitHub
          </a>
          <Link to="/terms">License</Link>
          <Link to="/privacy">Privacy</Link>
          <Link to="/terms">Terms</Link>
        </div>
        <p>© {new Date().getFullYear()} HavenKeys</p>
      </div>
    </footer>
  );
}

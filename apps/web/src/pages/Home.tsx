import { Link } from "react-router-dom";

const FEATURES = [
  {
    title: "Logins & autofill",
    body: "Save usernames and passwords once. The browser extension suggests them only on the sites they were saved for — never automatically, never on a look-alike domain.",
  },
  {
    title: "Secure notes",
    body: "Arbitrary encrypted text for anything that isn't a login: recovery codes, passphrases, whatever needs to stay off a screenshot.",
  },
  {
    title: "TOTP codes",
    body: "Import an otpauth:// URI once; HavenKeys generates the six- or eight-digit code from then on, in Rust, without ever handing the secret to a webpage.",
  },
  {
    title: "Password generator",
    body: "Cryptographically random and unbiased, generated in Rust — length and character sets are yours to configure.",
  },
  {
    title: "Your own server",
    body: "havenkeys-server is a small server you run yourself. It stores ciphertext it cannot open, and it's the only thing keeping your devices in sync.",
  },
  {
    title: "Browser extension",
    body: "Manifest V3, minimal permissions, and every fill request checked against the current page's origin in Rust — not trusted from the extension.",
  },
];

export function Home() {
  return (
    <>
      <section className="hero">
        <p className="hero__eyebrow">Local-first password manager</p>
        <h1>Your passwords, encrypted before they ever leave your device.</h1>
        <p className="hero__lede">
          HavenKeys pairs a Tauri desktop app with a Rust security core, a browser extension, and
          a small server you run yourself. Keys and plaintext never leave your device — the
          server only ever holds ciphertext it has no way to open.
        </p>
        <div className="hero__actions">
          <Link to="/download" className="button button--primary">
            Download HavenKeys
          </Link>
          <a
            className="button button--secondary"
            href="https://github.com/rochasamuel/havenkeys"
            target="_blank"
            rel="noreferrer"
          >
            View on GitHub
          </a>
        </div>
      </section>

      <section className="how-it-works">
        <h2>How it's built</h2>
        <ol>
          <li>
            Your master password runs through <code>Argon2id</code> to derive a master key — it
            is never used as an encryption key directly.
          </li>
          <li>
            The master key unwraps a key-encryption key, which unwraps a randomly generated vault
            key.
          </li>
          <li>
            Every item is encrypted individually with <code>AES-256-GCM</code> under the vault
            key, with a unique nonce every time.
          </li>
          <li>
            The desktop UI never does cryptography and never sees your master password after
            unlock — that stays in the Rust core.
          </li>
        </ol>
      </section>

      <section className="features">
        <h2>What it does</h2>
        <div className="features__grid">
          {FEATURES.map((feature) => (
            <div className="feature-card" key={feature.title}>
              <h3>{feature.title}</h3>
              <p>{feature.body}</p>
            </div>
          ))}
        </div>
      </section>

      <section className="security-teaser">
        <h2>Security, not marketing</h2>
        <p>
          We'd rather point you at the actual documents than summarize them: the{" "}
          <Link to="/security">security page</Link> links straight to the threat model, the
          security model, and the cryptography design.
        </p>
        <p>
          This software has not undergone an independent security audit and should not be
          considered a replacement for professionally audited password managers for high-value
          production use.
        </p>
      </section>
    </>
  );
}

export function Privacy() {
  return (
    <section className="docs-page">
      <h1>Privacy Policy</h1>
      <p className="docs-page__updated">Last updated September 22, 2026.</p>

      <h2>This website</h2>
      <p>
        havenkeys.net sets no cookies. It uses Vercel Analytics, which reports which pages were
        viewed in aggregate without a persistent identifier or cross-site tracking. There is no
        advertising and no third-party tracking script of any kind.
      </p>

      <h2>The desktop app and browser extension</h2>
      <p>
        HavenKeys the application contains no telemetry, no analytics, no crash reporting, and no
        accounts system in this version. Your master password never leaves your device and is
        never transmitted anywhere, including to the browser extension. Vault keys are generated
        on your device.
      </p>
      <p>
        If you choose to run <code>havenkeys-server</code> yourself and connect a device to it,
        that server stores encrypted vault data it cannot decrypt, plus the minimum routing
        metadata described in the{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/docs/server-sync.md"
          target="_blank"
          rel="noreferrer"
        >
          server-sync design
        </a>
        . That server is one you host and control — we do not operate a hosted version and have
        no access to it.
      </p>

      <h2>What we never collect</h2>
      <ul>
        <li>Master passwords</li>
        <li>Vault encryption keys</li>
        <li>Stored passwords, usernames, TOTP secrets, or secure notes</li>
        <li>Browsing history or the contents of pages you visit</li>
      </ul>

      <h2>Contact</h2>
      <p>
        Questions about this policy can be opened as an issue on{" "}
        <a href="https://github.com/rochasamuel/havenkeys/issues" target="_blank" rel="noreferrer">
          GitHub
        </a>
        .
      </p>
    </section>
  );
}

export function Privacy() {
  return (
    <section className="docs-page">
      <h1>Privacy Policy</h1>
      <p className="docs-page__updated">Last updated September 22, 2026.</p>

      <h2>This website</h2>
      <p>
        havenkeys.net sets no cookies. It is hosted on Vercel, which processes standard request
        logs (such as IP address and browser user agent) to serve the site, and it uses Vercel
        Analytics, which counts page views in aggregate without cookies or cross-site tracking.
        The Download page asks GitHub's public API for the latest release directly from your
        browser, so GitHub also sees that request. There is no advertising and no other
        third-party script.
      </p>

      <h2>The desktop app and browser extension</h2>
      <p>
        HavenKeys the application contains no telemetry, no analytics, and no crash reporting.
        Your master password and Secret Key are never sent to the server or to the browser
        extension; to sign in, the app sends the server a key derived from them. Your Secret Key
        leaves your device only in ways you choose: on your Emergency Kit, and when you enter it
        on another device of your own. Vault keys are generated on your device and never leave it
        unencrypted.
      </p>
      <p>
        Every vault belongs to an account on a <code>havenkeys-server</code> that you, or whoever
        invited you, runs. That server stores your vault encrypted, which it cannot decrypt, plus
        the metadata it needs to serve it: the vault ID, your account's email address, the
        key-derivation parameters and salt, the wrapped vault key, item revisions, and the number
        and rough size of your items. To rate-limit sign-in, it also counts failed attempts per
        account and per network address, and clears them after a successful sign-in. The{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/docs/server-sync.md"
          target="_blank"
          rel="noreferrer"
        >
          server-sync design
        </a>{" "}
        describes this in full. We do not operate a hosted server and have no access to yours.
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

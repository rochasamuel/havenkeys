export function Terms() {
  return (
    <section className="docs-page">
      <h1>Terms of Service</h1>
      <p className="docs-page__updated">Last updated September 22, 2026.</p>

      <h2>License</h2>
      <p>
        HavenKeys is open-source software, dual-licensed under the{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/LICENSE-MIT"
          target="_blank"
          rel="noreferrer"
        >
          MIT License
        </a>{" "}
        and the{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/LICENSE-APACHE"
          target="_blank"
          rel="noreferrer"
        >
          Apache License 2.0
        </a>
        . You may use, modify, and redistribute it under the terms of either.
      </p>

      <h2>No warranty</h2>
      <p>
        HavenKeys is provided "as is," without warranty of any kind, express or implied,
        including but not limited to fitness for a particular purpose. This software has not
        undergone an independent security audit and should not be considered a replacement for
        professionally audited password managers for high-value production use. You use it at
        your own risk.
      </p>

      <h2>Self-hosting</h2>
      <p>
        If you run <code>havenkeys-server</code> yourself, you are solely responsible for its
        deployment, its uptime, and its backups. The server is the authoritative copy of your
        vault; losing it without a tested backup means losing your data. See{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/docs/deployment.md"
          target="_blank"
          rel="noreferrer"
        >
          the deployment guide
        </a>
        , particularly its section on backups, before storing anything you can't afford to lose.
      </p>

      <h2>No service, no account</h2>
      <p>
        We do not operate a hosted version of HavenKeys and do not maintain accounts on your
        behalf. There is no subscription, no SLA, and no support obligation implied by
        downloading this software.
      </p>
    </section>
  );
}

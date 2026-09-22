const DOCS = [
  {
    title: "Threat model",
    href: "https://github.com/rochasamuel/havenkeys/blob/main/docs/threat-model.md",
    body: "What HavenKeys defends against, and what it explicitly doesn't.",
  },
  {
    title: "Security model",
    href: "https://github.com/rochasamuel/havenkeys/blob/main/docs/security-model.md",
    body: "How the defenses in the threat model are actually enforced, permission by permission.",
  },
  {
    title: "Cryptography",
    href: "https://github.com/rochasamuel/havenkeys/blob/main/docs/crypto.md",
    body: "The key hierarchy, the encrypted-blob format, and the exact Argon2id and AES-256-GCM parameters.",
  },
  {
    title: "Security review",
    href: "https://github.com/rochasamuel/havenkeys/blob/main/docs/security-review.md",
    body: "Findings from reviewing this implementation against its own threat model, including what's still open.",
  },
];

export function Security() {
  return (
    <section className="docs-page">
      <h1>Security</h1>
      <p>
        HavenKeys has not undergone an independent security audit. Rather than restate its design
        here, these are the actual documents that describe it — read them before trusting it with
        anything.
      </p>
      <ul className="docs-list">
        {DOCS.map((doc) => (
          <li key={doc.href}>
            <a href={doc.href} target="_blank" rel="noreferrer">
              {doc.title}
            </a>
            <p>{doc.body}</p>
          </li>
        ))}
      </ul>
    </section>
  );
}

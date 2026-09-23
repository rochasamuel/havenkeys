# Security policy

HavenKeys is a personal password manager maintained by one person. It has
**not** undergone an independent security audit. Treat it as experimental and
don't use it as your only store for high-value secrets.

## Reporting a vulnerability

**Please do not open a public issue, pull request or discussion for a
security problem.**

Report it privately through GitHub:
**Security → Report a vulnerability** on this repository
(GitHub private vulnerability reporting).

Please include:

* The affected component (core, desktop, extension, native host, server)
  and version or commit
* What an attacker needs (a malicious webpage, a local user, a copy of the
  vault database, control of the server, …)
* Steps to reproduce or a proof of concept
* What the attacker gains

Don't include real passwords, vault files or Secret Keys. Use a throwaway
vault.

## What to expect

This is a volunteer project, so the timelines below are what I aim for, not
a guarantee:

* An acknowledgement within 7 days
* An assessment of whether it's valid and how severe it is within 30 days
* A fix, or a documented mitigation in
  [docs/security-review.md](docs/security-review.md), before any public
  disclosure

I'll credit you in the advisory unless you ask me not to. Please give me a
reasonable chance to fix the problem before you disclose it; 90 days is the
default.

## Supported versions

Only the latest release and `main` receive security fixes.

## Scope

In scope, in particular:

* Recovering plaintext from the vault database, the server's stored data or
  network traffic without the master password and the Secret Key
* A webpage obtaining credentials, TOTP codes or vault metadata, or making the
  extension fill on an origin the item doesn't match
* Bypassing origin binding, the lock state, or native-messaging validation
* Secrets appearing in logs, errors, URLs, extension storage or the DOM
* The server learning plaintext, or one account reading or changing another's
  data

Already documented, and not in scope unless you show a way around the
stated mitigation:

* The limitations accepted in [docs/security-review.md](docs/security-review.md)
  and [docs/threat-model.md](docs/threat-model.md). Examples: a malicious
  process running as the same OS user, memory zeroization limits, the server
  being able to delete or roll back data, and clipboard exposure to other apps.
* A compromised operating system, browser or hardware

Out of scope:

* Denial of service against a server you don't operate
* Social engineering
* Reports from automated scanners without a demonstrated impact

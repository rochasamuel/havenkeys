# Autofill

> Status: the **domain matching and origin binding** described here are
> implemented and tested in the Rust core (`crates/havenkeys-core/src/origin.rs`,
> `VaultService::{find_matches, fill_for_page, totp_for_page}`). The browser
> extension (field detection, suggestion UI, filling) is not built yet.

## Domain matching rules

Every login has a list of website rules. Each rule is a normalized `http(s)`
URL plus a match type. Matching a page against a rule works like this:

1. **The page must be an `http` or `https` URL with a host.** `about:blank`,
   `data:`, `file:`, `javascript:`, `blob:`, and extension pages never match.
2. **No scheme downgrade.** An `https` rule never matches an `http` page. An
   `http` rule matches both `http` and `https` pages.
3. **Same port** (after applying scheme defaults).
4. **Then, depending on the match type:**

| Match type | UI label | Matches when | Strength |
|---|---|---|---|
| `exact` | This exact page | Same host and same path; query and fragment ignored | `exact_url` |
| `origin` | This exact site | Same host | `same_host` |
| `domain` (default) | Whole site, any subdomain | Same host, **or** same registrable domain (eTLD+1 from the Public Suffix List) | `same_host` / `same_site` |

A `domain` rule falls back to same-host matching when either side has no
registrable domain under a known public suffix. That covers IP addresses,
`localhost`, unknown TLDs such as `.internal`, and hosts that are themselves
public suffixes, such as `github.io`.

Suggestions are ranked `exact_url` → `same_host` → `same_site`, then by title.

### Examples (all are unit tests)

Rule `https://github.com`, match type `domain`:

| Page | Result |
|---|---|
| `https://github.com/login` | ✅ same host |
| `https://gist.github.com/` | ✅ same site |
| `http://github.com/` | ❌ scheme downgrade |
| `https://github.com.evil.com/` | ❌ different registrable domain |
| `https://github-login.example.com/` | ❌ |
| `https://evilgithub.com/` | ❌ |
| `https://github.com@evil.com/` | ❌ host is `evil.com` |
| `https://evil.com/?next=https://github.com` | ❌ |
| `https://gіthub.com/` (Cyrillic і) | ❌ different punycode host |
| `https://github.com:8443/` | ❌ different port |

Public-suffix boundaries:

| Rule | Page | Result |
|---|---|---|
| `https://alice.github.io` | `https://bob.github.io/` | ❌ each `github.io` site is separate |
| `https://bank.co.uk` | `https://evil.co.uk/` | ❌ |
| `https://bank.co.uk` | `https://login.bank.co.uk/` | ✅ same site |
| `http://192.168.1.1` | `http://192.168.1.2/` | ❌ IPs are host-only |

Hosts are compared after WHATWG URL parsing, so case, IDN punycode,
percent-encoding and a single trailing dot are all normalized first. No
matching is ever done with string prefixes, suffixes or regular expressions on
raw URLs.

## Origin binding (enforced in Rust)

The extension is never trusted to decide which item belongs to a page. For
every request the core re-derives the answer from the item's own rules:

| Core function | Returns | Check |
|---|---|---|
| `find_matches(page_url)` | ID, title, username, has-TOTP flag, strength. **No secrets.** | Only logins whose rules match the page |
| `fill_for_page(id, page_url)` | Username and password only | `Denied` unless the item is a login whose rules match the page |
| `totp_for_page(id, page_url, now)` | Current code only; the secret never leaves | Same as above |

All three return `Locked` when the vault is locked. Secure notes are never
served to pages. Regression tests A1 and A2 in
`crates/havenkeys-core/tests/security.rs` cover requesting the github.com item
from evil.com, look-alike hosts and downgrades, and requesting another site's
item ID.

The native-messaging layer (Phase 4) will pass the page URL as reported by the
browser (the tab or frame URL from the extension background worker, never
from page script) to these functions, and add rate limiting and explicit user
confirmation as described in CLAUDE.md §25.

## Not yet implemented

Field classification, MutationObserver scanning, the suggestion UI, explicit
fill, save-login detection, and iframe policy. These are Phase 5, and this
document will be extended as they land.

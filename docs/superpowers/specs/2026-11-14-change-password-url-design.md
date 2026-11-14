# Change password on the website — design

Date: 2026-11-14. Status: implemented (desktop).

## 1. Goal

A login's detail view, and the weak, reused and old rows of Vault health,
offer **Change on website**: one click opens the site's change-password page
in the default browser, as 1Password's "Change password" does. Finding that
page is the tedious part of fixing a weak or reused password; HavenKeys
already knows the site.

The page is found in Rust, in this order:

1. **Curated table, no network.** A site → change-password page map compiled
   into the core.
2. **The W3C well-known URL, on the click only.** The desktop asks the site
   for `https://<host>/.well-known/change-password` ("A Well-Known URL for
   Changing Passwords", W3C), after checking that the site does not answer
   2xx for a URL that cannot exist.
3. **The login's website.** With neither, the first saved website opens and
   the app says so.

## 2. Decisions taken

| Question | Decision |
|---|---|
| Curated data | Apple's Password Manager Resources, `quirks/change-password-URLs.json` (MIT), converted to `crates/havenkeys-core/data/change-password-urls.json` by `scripts/update-change-password-urls.mjs`; credited in `THIRD-PARTY-NOTICES.md`; never fetched at runtime |
| Lookup | The login's websites in order; for each, the exact host, then each parent domain down to its registrable domain (PSL), by DNS labels |
| A key that is itself a public suffix (`gov.br`) | Matches only that exact host, never every site under the suffix |
| Probe target | The first website whose host is a DNS name under a known public suffix, on the default port, as `https://<host>/` whatever the saved scheme |
| Not probed | IP addresses, `localhost`, single labels, private or unknown suffixes (`.local`, `.internal`, `.lan`), explicit ports |
| Probe success | `NOT_EXIST_PATH` answers a non-2xx (after redirects), then `/.well-known/change-password` ends in a 2xx |
| What opens on success | The well-known URL itself, so the browser follows the site's redirect with the user's own session; never an address from a response |
| When the probe runs | Only on the user's click on Change on website; never in the background, on unlock, in Vault health's report or for a list |
| Where | Desktop only (`apps/desktop/src-tauri/src/change_password.rs`). Not the extension or Android in this version |
| Renderer input | The item id only. The renderer never sends or sees an address |
| Label | "Change on website" / "Trocar no site". Vault health keeps its "Change password" (opens the editor) and gains this button beside it |

## 3. Core (`crates/havenkeys-core/src/change_password.rs`)

No network. Public functions:

* `plan(&ItemOverview) -> Option<ChangePasswordPlan>`: `None` unless the
  item is a login with at least one http(s) website. The plan holds the
  curated page (first website with one), the probe origin (first website
  that may be probed) and the fallback (the first website, through
  `model::normalize_url`, the check saving uses).
* `curated_url(website)`, `probe_origin(website)`.
* `probe_may_request(&Url)`: https, default port, no credentials, a DNS name
  under a known public suffix. The desktop checks the first request and
  every redirect with it.
* `is_public_address(IpAddr)`: globally routed unicast only. Refused:
  unspecified, loopback, RFC 1918, link-local (including 169.254.169.254),
  CGNAT 100.64/10, 192.0.0/24, documentation, benchmarking 198.18/15,
  multicast, broadcast, 240/4; IPv6 loopback, ULA fc00::/7, link-local,
  site-local, documentation, and the forms that embed an IPv4 address
  (mapped, compatible, NAT64 64:ff9b::/96, 6to4 2002::/16).
* Constants `WELL_KNOWN_PATH`, `NOT_EXIST_PATH`
  (`/.well-known/resource-that-should-not-exist-whose-status-code-should-not-be-200`),
  `MAX_REDIRECTS` (5).

The table is parsed once (`OnceLock`). An entry whose key is not a
lowercase DNS name exactly as the URL parser writes it, or whose page is not
an https URL with a DNS host and no credentials, is dropped, never a panic.
A test requires the bundled file to have no such entry and every key to be
under a known public suffix.

## 4. Desktop command

`open_change_password(id) -> "curated" | "well_known" | "website"`:

1. `touch()`; the vault must be unlocked; `get_item(id)`; `plan` (`not_found`
   for a non-login or a login without a website).
2. The vault lock is released before any network access.
3. Curated page → open. Else probe origin → probe → on success open the
   well-known URL. Else open the fallback website.
4. Open with `tauri_plugin_opener::open_url`, as `open_website` does.

The renderer is granted only `allow-open-change-password`
(`capabilities/main.json`); the Quick Access window is not.

### 4.1 The probe

A `reqwest` 0.12 client (already in the tree through
`havenkeys-sync-client`, same features: rustls, no cookie store), built per
click:

* `https_only`, `no_proxy`, `referer(false)`, User-Agent `havenkeys` (the
  value the app's other requests use), no `Authorization`, no cookies.
* Connect timeout 3 s, 4 s per request, 8 s for the whole probe.
* A redirect policy that follows at most 5 redirects and stops (yielding the
  3xx, which is not a 2xx) at any target failing `probe_may_request`.
* A DNS resolver that resolves with the system resolver and keeps only
  `is_public_address` answers; a name with none fails. This also covers DNS
  rebinding between the two requests, since every connection resolves
  through it.
* Neither body is read. Any error, timeout or non-2xx is "no signal".

### 4.2 What the site learns

The same as when the user opens the site, and less: the user's IP address,
that `havenkeys` asked for these two paths, and the time. No cookie, no
credential, no Referer, nothing from the vault. It runs only after the
user's click on that login, so it reveals nothing the user was not about to
reveal by visiting the site. Nobody else is contacted: no HavenKeys server,
no third-party list or favicon service.

## 5. UI

* **Item detail** (logins with an http(s) website): a small secondary
  button with the key glyph, before Edit in the header. It is not disabled
  when the app is read-only (offline from the server or frozen): it saves
  nothing. While Rust works it is disabled (`aria-busy`). When the result is
  `website`, a toast says the site points to no change-password page, so
  its website opened. Errors use the "Could not open the website." toast.
* **Vault health**: the same button on rows whose fix is a new password
  (weak, reused, old), after "Change password", for logins with a website.
* Strings: `detail.changeOnWebsite`, `detail.changeOnWebsiteHint`,
  `detail.changePageNotFound` (en, pt-BR).

## 6. Tests

* Core: bundled-table validity in full; exact host then registrable domain;
  look-alikes never match; a parent of a listed subdomain does not match; a
  public-suffix key matches only itself; hostile entries dropped; probe
  origin and `probe_may_request` refusals; public and non-public addresses;
  `plan` order and the non-login case.
* Desktop (axum on 127.0.0.1, with test-only rules allowing local http): a
  redirecting well-known URL is found and the well-known URL (not the
  target) is returned; a site answering 200 to everything, and one that
  redirects every unknown path to a 200 page, give no signal; 404 and 500
  are not found; 5 redirects are followed, 6 are not; slow sites time out
  (per request and overall); an unreachable port; no Cookie (even after a
  `Set-Cookie`), Authorization or Referer is sent; the production client
  never connects to loopback, `localhost` or plain http (a counting
  listener sees no connection); the public resolver refuses `localhost`;
  the curated page wins without a probe, then the probe, then the website.
* UI (vitest): the button calls the command with the item id only, works
  while read-only, is disabled while pending, toasts the website fallback,
  and is absent without a website or on a note; the health row shows it
  for a weak password and not for a passkey row.

## 7. Not in this version

* The browser extension and Android. Both could reuse `plan`; Android's
  probe would live next to `asset_links_fetch.rs`.
* Refreshing the table automatically; it is refreshed by the script and
  reviewed like code.
* Following a login's equivalent domains (`equivalent-domains.json`) when
  looking up the table.

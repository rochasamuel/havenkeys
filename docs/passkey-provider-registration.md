# Registering HavenKeys as a passkey provider

Sites show which manager holds a passkey ("HavenKeys", with its icon) by
looking up the **AAGUID** in the passkey's attested credential data. HavenKeys
creates every passkey with its own AAGUID,
`5fa9623b-c2ff-4166-92da-c011a036bcb0`
(`crates/havenkeys-core/src/passkey/webauthn.rs`, `AAGUID`).

> This software has not undergone an independent security audit.

A site cannot turn that ID into a name unless it is published. Sites read one
of two sources:

| Source | Used by | HavenKeys status |
|---|---|---|
| Community registry [`passkeydeveloper/passkey-authenticator-aaguids`](https://github.com/passkeydeveloper/passkey-authenticator-aaguids) (`aaguid.json`) | Google Account, many RPs and libraries (SimpleWebAuthn examples, Okta, Auth0-style dashboards) | **Not listed** (checked 2026-11-14: 67 entries, including 1Password `bada5566-a7aa-401f-bd96-45619a55120d`, Bitwarden, Dashlane, Keeper, Proton Pass) |
| FIDO Alliance Metadata Service (MDS3) | Enterprise RPs that validate authenticators | Not listed. Needs FIDO membership/certification; software passkey managers with `none` attestation are normally not in MDS |

Until it is listed, a site shows HavenKeys passkeys as an unknown
authenticator or a generic security key. Before the AAGUID existed, HavenKeys
used the all-zero AAGUID, which Google labelled as Apple's.

## Submitting to the community registry

1. Fork `passkeydeveloper/passkey-authenticator-aaguids`.
2. Add this entry to `aaguid.json` (keep the file sorted as the README asks):

```json
{
  "5fa9623b-c2ff-4166-92da-c011a036bcb0": {
    "name": "HavenKeys",
    "icon_dark": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNDAiIGhlaWdodD0iMjQwIiB2aWV3Qm94PSIwIDAgMTAyNCAxMDI0Ij48cmVjdCB3aWR0aD0iMTAyNCIgaGVpZ2h0PSIxMDI0IiByeD0iMjI0IiBmaWxsPSIjMTQyMDFjIi8+PHBhdGggZD0iTTUxMiAxOTYgTDc3MiAzMDAgVjUwMCBDNzcyIDY2OCA2NjIgNzkyIDUxMiA4NDYgQzM2MiA3OTIgMjUyIDY2OCAyNTIgNTAwIFYzMDAgWiIgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZThjMzdhIiBzdHJva2Utd2lkdGg9IjQ0IiBzdHJva2UtbGluZWpvaW49InJvdW5kIi8+PGNpcmNsZSBjeD0iNTEyIiBjeT0iNDY4IiByPSI4NCIgZmlsbD0iI2U4YzM3YSIvPjxyZWN0IHg9IjQ4NiIgeT0iNTIwIiB3aWR0aD0iNTIiIGhlaWdodD0iMTcwIiByeD0iMTgiIGZpbGw9IiNlOGMzN2EiLz48L3N2Zz4=",
    "icon_light": "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNDAiIGhlaWdodD0iMjQwIiB2aWV3Qm94PSIwIDAgMTAyNCAxMDI0Ij48cmVjdCB3aWR0aD0iMTAyNCIgaGVpZ2h0PSIxMDI0IiByeD0iMjI0IiBmaWxsPSIjMTQyMDFjIi8+PHBhdGggZD0iTTUxMiAxOTYgTDc3MiAzMDAgVjUwMCBDNzcyIDY2OCA2NjIgNzkyIDUxMiA4NDYgQzM2MiA3OTIgMjUyIDY2OCAyNTIgNTAwIFYzMDAgWiIgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZThjMzdhIiBzdHJva2Utd2lkdGg9IjQ0IiBzdHJva2UtbGluZWpvaW49InJvdW5kIi8+PGNpcmNsZSBjeD0iNTEyIiBjeT0iNDY4IiByPSI4NCIgZmlsbD0iI2U4YzM3YSIvPjxyZWN0IHg9IjQ4NiIgeT0iNTIwIiB3aWR0aD0iNTIiIGhlaWdodD0iMTcwIiByeD0iMTgiIGZpbGw9IiNlOGMzN2EiLz48L3N2Zz4="
  }
}
```

   The icon is the app's mark (`apps/web/src/components/Mark.tsx`) as a
   240×240 SVG on its own dark tile, so the same image works on light and dark
   backgrounds. Replace either icon if the brand needs separate versions.
3. Open the pull request from the HavenKeys project account, with a link to
   the product site and to this repository's AAGUID constant as evidence.
4. After it merges, check `https://myaccount.google.com/signinoptions/passkeys`
   shows "HavenKeys" for a passkey created afterwards (Google reads the
   registry; existing passkeys keep the AAGUID they were created with, which is
   already HavenKeys').

The AAGUID must never change: every passkey ever created carries it.

## Other signals sites use (implemented in the extension and Android)

* `authenticatorAttachment: "platform"`, backup-eligible and backed-up flags
  (BE/BS) set: a synced passkey, not a security key.
* `credProps: { rk: true }` when the site asks: the credential is
  discoverable (a passkey). Without it, many sites list the credential as a
  "security key".
* `transports: ["hybrid", "internal"]`, as synced platform passkeys report.
* `isUserVerifyingPlatformAuthenticatorAvailable()`,
  `isConditionalMediationAvailable()` and `getClientCapabilities()` answer
  that a platform passkey store is present.

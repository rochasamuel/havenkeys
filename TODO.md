# TODO

Open tasks that need a person (accounts, outside services, decisions), not
just code. Details live in the linked docs.

## Register HavenKeys as a passkey provider

Sites (Oracle, Google Account, …) show HavenKeys passkeys as an unknown
authenticator or a generic security key because HavenKeys' AAGUID
`5fa9623b-c2ff-4166-92da-c011a036bcb0` is not in the public AAGUID registry.
Full background, the ready-to-paste entry and the icon:
[`docs/passkey-provider-registration.md`](docs/passkey-provider-registration.md).

- [ ] Fork [`passkeydeveloper/passkey-authenticator-aaguids`](https://github.com/passkeydeveloper/passkey-authenticator-aaguids)
      from the HavenKeys project account.
- [ ] Add the HavenKeys entry (name + `icon_dark` + `icon_light`) from the doc
      to `aaguid.json`, keeping the file sorted as its README asks.
- [ ] Check the icon renders on light and dark backgrounds; replace it if the
      brand needs separate versions.
- [ ] Open the pull request, linking the product site and
      `crates/havenkeys-core/src/passkey/webauthn.rs` (`AAGUID`) as evidence.
- [ ] After it merges, create a passkey on Google and confirm
      <https://myaccount.google.com/signinoptions/passkeys> shows "HavenKeys".
- [ ] Re-test Oracle: create a passkey and check how it is listed. If it still
      shows a security key, capture its page scripts and check whether it reads
      the AAGUID, `credProps` or `transports`.
- [ ] Decide whether FIDO MDS3 listing is worth pursuing (needs FIDO Alliance
      membership; software managers with `none` attestation are usually not
      listed).

Never change the AAGUID: every passkey already created carries it.

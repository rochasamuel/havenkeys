// The "Sign in with" row of a login's detail view. Which login it links to
// is decided in Rust (`provider_login`), the same rule a sign-in run uses.

import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { PROVIDER_NAMES } from "../lib/sso";
import type { ItemOverview, ProviderLogin } from "../lib/types";
import { useI18n } from "../i18n/context";
import { Field, IconButton } from "./Field";
import { ProviderIcon } from "./ProviderIcon";

export function SsoRow({ item, onOpen }: { item: ItemOverview; onOpen: (id: string) => void }) {
  const { t } = useI18n();
  const [link, setLink] = useState<ProviderLogin | null>(null);
  const sso = item.signInWith;

  useEffect(() => {
    let live = true;
    setLink(null);
    api.providerLogin(item.id).then(
      (l) => live && setLink(l),
      () => live && setLink(null),
    );
    return () => {
      live = false;
    };
  }, [item.id, item.updatedAt]);

  if (!sso) return null;
  const name = PROVIDER_NAMES[sso.provider];
  const note =
    sso.account && link?.kind === "none"
      ? t.detail.noProviderLogin(name)
      : sso.account && link?.kind === "several"
        ? t.detail.severalProviderLogins(name)
        : null;
  return (
    <Field
      label={t.detail.signInWith}
      actions={link?.kind === "one" ? <IconButton icon="arrowRight" label={t.detail.openProviderLogin(name)} onClick={() => onOpen(link.id)} /> : undefined}
    >
      <span className="sso-value">
        <ProviderIcon provider={sso.provider} />
        <span>{name}</span>
        {sso.account && <span className="muted selectable" data-truncate="">{sso.account}</span>}
      </span>
      {note && <span className="sso-note muted">{note}</span>}
    </Field>
  );
}

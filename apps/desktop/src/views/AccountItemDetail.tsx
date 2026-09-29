import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { AccountField, AccountStatus } from "../lib/types";
import { useRevealedSecret } from "../lib/hooks";
import { CopyButton } from "../components/CopyButton";
import { Seal } from "../components/Seal";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { Field, IconButton } from "./ItemDetail";

interface Props {
  account: AccountStatus;
  onShowKit: () => void;
}

/**
 * The HavenKeys Account item: the account's email, server, ID and Secret
 * Key, read-only. Nothing here is stored as a vault item; the values come
 * from the account record, and the Secret Key from this device, only when
 * revealed or copied. A revealed key lives in this component's state and
 * goes with it (on lock, or on choosing another item).
 */
export function AccountItemDetail({ account, onShowKit }: Props) {
  const toast = useToast();
  const { t } = useI18n();
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const secretKey = useRevealedSecret(useCallback(() => api.revealAccountSecretKey(), []));

  useEffect(() => {
    let cancelled = false;
    api.deviceStatus().then(
      (d) => !cancelled && setHasKey(d.secretKeyStorage !== "none"),
      () => !cancelled && setHasKey(null),
    );
    return () => {
      cancelled = true;
    };
  }, []);

  const copy = async (field: AccountField) => {
    try {
      const r = await api.copyAccountField(field);
      toast(t.accountItem.copied[field](r.clearAfterSeconds));
      return true;
    } catch (e) {
      toast(errorMessage(e, t, t.common.couldNotCopy), "error");
      return false;
    }
  };
  const copyButton = (field: AccountField) => (
    <CopyButton label={t.accountItem.copy[field]} onCopy={() => copy(field)} />
  );

  return (
    <article className="item">
      <header className="item-head" data-tauri-drag-region>
        <span className="avatar avatar-lg avatar-account" aria-hidden="true">
          <Seal size={40} />
        </span>
        <div className="item-head-text">
          <h2 className="item-title">{t.accountItem.title}</h2>
          <p className="item-kind">{t.accountItem.kind}</p>
        </div>
      </header>

      <div className="group">
        <Field label={t.common.email} actions={copyButton("email")}>
          <span className="selectable">{account.email}</span>
        </Field>
        <Field label={t.common.server} actions={copyButton("server")}>
          <span className="mono selectable">{account.serverUrl}</span>
        </Field>
      </div>

      <h3 className="group-title">{t.accountItem.secretKeyHeading}</h3>
      <div className="group">
        {hasKey === false ? (
          <div className="row">
            <span className="muted">{t.accountItem.notOnThisComputer}</span>
          </div>
        ) : (
          <Field
            label={t.common.secretKey}
            actions={
              <>
                <IconButton
                  icon={secretKey.value === null ? "eye" : "eyeOff"}
                  label={secretKey.value === null ? t.accountItem.showSecretKey : t.accountItem.hideSecretKey}
                  onClick={() =>
                    secretKey.value === null
                      ? void secretKey
                          .reveal()
                          .catch((e) => toast(errorMessage(e, t, t.common.couldNotReveal), "error"))
                      : secretKey.hide()
                  }
                />
                {copyButton("secret_key")}
              </>
            }
          >
            <span className="secret" data-revealed={secretKey.value !== null}>
              {secretKey.value === null ? (
                <span className="mono masked" aria-label={t.accountItem.hiddenSecretKey}>
                  ••••••••••••
                </span>
              ) : (
                <span className="mono selectable revealed">{secretKey.value}</span>
              )}
            </span>
          </Field>
        )}
      </div>

      <div className="group">
        <Field label={t.accountItem.accountId} actions={copyButton("account_id")}>
          <span className="mono selectable">{account.accountId}</span>
        </Field>
      </div>

      <div className="group">
        <div className="row account-item-note">
          <p>{t.accountItem.note}</p>
          <button className="btn btn-small" type="button" onClick={onShowKit}>
            {t.accountItem.showKit}
          </button>
        </div>
      </div>

      <footer className="item-foot">
        <p className="muted">{t.accountItem.readOnly}</p>
      </footer>
    </article>
  );
}

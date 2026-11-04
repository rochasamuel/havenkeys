import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { BankCopyField, BankView, ItemOverview } from "../lib/types";
import { joinDigit } from "../lib/bank";
import { formatDate } from "../lib/format";
import { useRevealedSecret } from "../lib/hooks";
import { CopyButton } from "../components/CopyButton";
import { Attachments } from "../components/Attachments";
import { Field, IconButton } from "../components/Field";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { CustomSections } from "./CustomSections";
import { ItemAvatar } from "./ItemList";
import { Websites } from "./LoginParts";

interface Props {
  item: ItemOverview;
  /** Offline: editing and deleting would fail, so the controls are disabled up front. */
  readOnly: boolean;
  onEdit: () => void;
  /** Open the vault filtered by this tag. */
  onTag?: (tag: string) => void;
  onDelete: () => void;
}

/** The account and its digit as one value; an account without a digit has none to reveal. */
async function revealAccount(id: string): Promise<string> {
  const account = await api.revealBankField(id, "account");
  const digit = await api.revealBankField(id, "accountDigit").catch((e: unknown) => {
    if (e instanceof ApiError && e.code === "not_found") return null;
    throw e;
  });
  return joinDigit(account, digit);
}

/**
 * The account or the password: dots until the eye, fetched from Rust only
 * then, and dropped when hidden, after 30 s, and on unmount (including lock).
 */
function SecretRow(props: {
  label: string;
  masked: string;
  /** What a screen reader hears for the dots. */
  hiddenLabel: string;
  load: () => Promise<string>;
  copy: () => Promise<boolean>;
}) {
  const { label, masked, hiddenLabel, load, copy } = props;
  const { t } = useI18n();
  const toast = useToast();
  const secret = useRevealedSecret(load);
  const shown = secret.value !== null;
  return (
    <Field
      label={label}
      actions={
        <>
          <IconButton
            icon={shown ? "eyeOff" : "eye"}
            label={shown ? t.bank.hide(label) : t.bank.show(label)}
            onClick={() =>
              shown ? secret.hide() : void secret.reveal().catch((e) => toast(errorMessage(e, t, t.bank.revealFailed), "error"))
            }
          />
          <CopyButton label={t.bank.copy(label)} onCopy={copy} />
        </>
      }
    >
      <span className="secret" data-revealed={shown}>
        {shown ? (
          <span className="mono selectable revealed">{secret.value}</span>
        ) : (
          <span className="mono masked" aria-label={hiddenLabel}>
            {masked}
          </span>
        )}
      </span>
    </Field>
  );
}

/** A bank account, read-only. Opening it decrypts everything but the account, its digit and the password. */
export function BankDetail({ item, readOnly, onTag, onEdit, onDelete }: Props) {
  const toast = useToast();
  const { t, locale, dateLocale } = useI18n();
  const [view, setView] = useState<BankView | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api.revealBank(item.id).then(
      (v) => !cancelled && setView(v),
      (e) => {
        if (cancelled) return;
        setFailed(true);
        toast(errorMessage(e, t, t.bank.loadFailed), "error");
      },
    );
    return () => {
      cancelled = true;
      setView(null);
    };
    // `t` is left out: a language change must not decrypt the account again.
  }, [item.id, toast]);

  const copy = (field: BankCopyField, label: string) => async () => {
    try {
      const r = await api.copyBankField(item.id, field, locale);
      toast(t.bank.copied(label, r.clearAfterSeconds));
      return true;
    } catch (e) {
      toast(errorMessage(e, t, t.common.couldNotCopy), "error");
      return false;
    }
  };
  const copyButton = (field: BankCopyField, label: string) => <CopyButton label={t.bank.copy(label)} onCopy={copy(field, label)} />;
  const loadAccount = useCallback(() => revealAccount(item.id), [item.id]);
  const loadPassword = useCallback(() => api.revealBankField(item.id, "password"), [item.id]);
  const f = t.bank.fields;
  const last3 = item.bank?.accountLast3;

  return (
    <article className="item">
      <header className="item-head" data-tauri-drag-region>
        <ItemAvatar item={item} large />
        <div className="item-head-text">
          <h2 className="item-title">{item.title}</h2>
          <p className="item-kind">{t.bank.kind}</p>
        </div>
        <div className="item-head-actions">
          <button className="btn btn-small" onClick={() => void copy("details", t.bank.details)()} disabled={!view}>
            <Icon name="copy" size={15} /> {t.bank.copyDetails}
          </button>
          <button className="btn btn-small" onClick={onEdit} disabled={readOnly || !view}>
            <Icon name="edit" size={15} /> {t.common.edit}
          </button>
        </div>
      </header>

      {item.tags.length > 0 && (
        <div className="detail-tags" role="group" aria-label={t.detail.tags}>
          {item.tags.map((name) => (
            <button key={name} type="button" className="chip tag-chip" onClick={() => onTag?.(name)}>
              <Icon name="tag" size={12} />
              {name}
            </button>
          ))}
        </div>
      )}

      {!view ? (
        !failed && <p className="muted">{t.common.decrypting}</p>
      ) : (
        <>
          <div className="group">
            {view.bank && (
              <Field label={f.bank} actions={copyButton("bank", f.bank)}>
                <span className="selectable" data-truncate="" title={view.bank}>
                  {view.bank}
                </span>
              </Field>
            )}
            {view.branch && (
              <Field label={f.branch} actions={copyButton("branch", f.branch)}>
                <span className="mono selectable">{joinDigit(view.branch, view.branchDigit)}</span>
              </Field>
            )}
            {view.hasAccount && (
              <SecretRow
                label={f.account}
                masked={last3 ? `•••${last3}` : "•••"}
                hiddenLabel={last3 ? t.bank.accountEnding(last3) : t.bank.hidden(f.account)}
                load={loadAccount}
                copy={copy("account", f.account)}
              />
            )}
            {view.accountType && (
              <Field label={f.accountType}>
                <span>{t.bank.types[view.accountType]}</span>
              </Field>
            )}
            {view.holderName && (
              <Field label={f.holderName} actions={copyButton("holderName", f.holderName)}>
                <span className="selectable" data-truncate="" title={view.holderName}>
                  {view.holderName}
                </span>
              </Field>
            )}
            {view.hasPassword && (
              <SecretRow label={f.password} masked="••••••••••••" hiddenLabel={t.bank.hidden(f.password)} load={loadPassword} copy={copy("password", f.password)} />
            )}
          </div>

          <Websites item={item} />
          {view.notes && (
            <section aria-label={f.notes}>
              <h3 className="group-title">{f.notes}</h3>
              <div className="group">
                <div className="row">
                  <p className="note-body selectable">{view.notes}</p>
                </div>
              </div>
            </section>
          )}
          <CustomSections itemId={item.id} updatedAt={item.updatedAt} />
        </>
      )}

      <Attachments item={item} />

      <footer className="item-foot">
        <p className="muted">
          {t.detail.dates(formatDate(item.createdAt, dateLocale), formatDate(item.updatedAt, dateLocale))}
        </p>
        <button className="btn btn-small btn-quiet-danger" onClick={onDelete} disabled={readOnly}>
          <Icon name="trash" size={15} /> {t.common.delete}
        </button>
      </footer>
    </article>
  );
}

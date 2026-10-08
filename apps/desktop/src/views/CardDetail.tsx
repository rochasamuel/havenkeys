import { useEffect, useRef, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import type { CardCopyField, CardRevealField, CardView, ItemOverview } from "../lib/types";
import { BRAND_NAMES, formatExpiry, groupNumber, isExpired, maskedNumber } from "../lib/card";
import { formatDate } from "../lib/format";
import { CardBrandLogo } from "../components/CardBrandLogo";
import { CopyButton } from "../components/CopyButton";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { Field, IconButton } from "../components/Field";

interface Props {
  item: ItemOverview;
  /** Offline: editing and deleting would fail, so the controls are disabled up front. */
  readOnly: boolean;
  onEdit: () => void;
  /** Open the vault filtered by this tag. */
  onTag?: (tag: string) => void;
  onDelete: () => void;
}

/**
 * The number or the verification number. Fetched from Rust only when the
 * eye is clicked; dropped when hidden again, after 30 s at most, and when
 * this unmounts (including on lock).
 */
function RevealableField(props: {
  id: string;
  field: CardRevealField;
  label: string;
  masked: string;
  format: (value: string) => string;
  copyButton: ReactNode;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const [value, setValue] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const pending = useRef(false);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const shown = value !== null;
  async function toggle() {
    // A second click while the value is being fetched is ignored.
    if (pending.current) return;
    window.clearTimeout(timer.current);
    if (shown) {
      setValue(null);
      return;
    }
    pending.current = true;
    try {
      setValue(await api.revealCardField(props.id, props.field));
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setValue(null), 30_000);
    } catch (e) {
      toast(errorMessage(e, t, t.card.revealFailed), "error");
    } finally {
      pending.current = false;
    }
  }
  return (
    <Field
      label={props.label}
      actions={
        <>
          <IconButton
            icon={shown ? "eyeOff" : "eye"}
            label={shown ? t.card.hide(props.label) : t.card.show(props.label)}
            onClick={() => void toggle()}
          />
          {props.copyButton}
        </>
      }
    >
      <span className="secret" data-revealed={shown}>
        {value !== null ? (
          <span className="mono selectable revealed">{props.format(value)}</span>
        ) : (
          <span className="mono masked" aria-label={t.card.hidden(props.label)}>
            {props.masked}
          </span>
        )}
      </span>
    </Field>
  );
}

/** A card, read-only. Opening it decrypts everything but the number and code. */
export function CardDetail({ item, readOnly, onTag, onEdit, onDelete }: Props) {
  const toast = useToast();
  const { t, dateLocale } = useI18n();
  const [view, setView] = useState<CardView | null>(null);
  const [failed, setFailed] = useState(false);
  const brand = item.card?.brand ?? "other";

  useEffect(() => {
    let cancelled = false;
    api.revealCard(item.id).then(
      (v) => !cancelled && setView(v),
      (e) => {
        if (cancelled) return;
        setFailed(true);
        toast(errorMessage(e, t, t.card.loadFailed), "error");
      },
    );
    return () => {
      cancelled = true;
      setView(null);
    };
    // `t` is left out: a language change must not decrypt the card again.
  }, [item.id, toast]);

  const copy = (field: CardCopyField, label: string) => async () => {
    try {
      const r = await api.copyCardField(item.id, field);
      toast(t.card.copied(label, r.clearAfterSeconds));
      return true;
    } catch (e) {
      toast(errorMessage(e, t, t.common.couldNotCopy), "error");
      return false;
    }
  };
  const copyButton = (field: CardCopyField, label: string) => (
    <CopyButton label={t.card.copy(label)} onCopy={copy(field, label)} />
  );
  const f = t.card.fields;
  const expiry = view?.expiry ?? item.card?.expiry ?? null;

  return (
    <article className="item">
      <header className="item-head" data-tauri-drag-region>
        <span className="avatar avatar-lg avatar-card" aria-hidden="true">
          <CardBrandLogo brand={brand} width={46} />
        </span>
        <div className="item-head-text">
          <h2 className="item-title">{item.title}</h2>
          <p className="item-kind">{t.card.kind}</p>
        </div>
        <div className="item-head-actions">
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
            {view.cardholderName && (
              <Field label={f.cardholderName} actions={copyButton("cardholderName", f.cardholderName)}>
                <span className="selectable" data-truncate="">
                  {view.cardholderName}
                </span>
              </Field>
            )}
            <Field label={f.brand}>
              <span className="card-type">
                <CardBrandLogo brand={brand} width={28} />
                <span>{brand === "other" ? t.card.otherBrand : BRAND_NAMES[brand]}</span>
              </span>
            </Field>
            {view.hasNumber && (
              <RevealableField
                id={item.id}
                field="number"
                label={f.number}
                masked={maskedNumber(item.card?.last4)}
                format={(v) => groupNumber(v, brand)}
                copyButton={copyButton("number", f.number)}
              />
            )}
            {view.hasVerificationNumber && (
              <RevealableField
                id={item.id}
                field="verificationNumber"
                label={f.verificationNumber}
                masked="•••"
                format={(v) => v}
                copyButton={copyButton("verificationNumber", f.verificationNumber)}
              />
            )}
            {expiry && (
              <Field label={f.expiry} actions={copyButton("expiry", f.expiry)}>
                <span className="mono selectable">{formatExpiry(expiry)}</span>
                {isExpired(expiry, new Date()) && <span className="card-expired">{t.card.expired}</span>}
              </Field>
            )}
          </div>

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
        </>
      )}

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

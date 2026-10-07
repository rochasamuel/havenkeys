import { useEffect, useRef, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import type { IdentityCopyField, IdentityView, ItemOverview } from "../lib/types";
import { formatBirthDate, identityTitle, isEmptyIdentity, visibleSections, type IdentityFieldDef } from "../lib/identity";
import { formatDate } from "../lib/format";
import { CopyButton } from "../components/CopyButton";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { Field, IconButton } from "../components/Field";

interface Props {
  item: ItemOverview;
  /** Offline: editing would fail, so the control is disabled up front. */
  readOnly: boolean;
  onEdit: () => void;
  /** Open the vault filtered by this tag. */
  onTag?: (tag: string) => void;
}

/** A masked field with an eye; hides itself again after 30 s, like a password. */
function MaskedField({ label, value, copyButton }: { label: string; value: string; copyButton: ReactNode }) {
  const { t } = useI18n();
  const [shown, setShown] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const toggle = () => {
    window.clearTimeout(timer.current);
    if (!shown) timer.current = window.setTimeout(() => setShown(false), 30_000);
    setShown(!shown);
  };
  return (
    <Field
      label={label}
      actions={
        <>
          <IconButton
            icon={shown ? "eyeOff" : "eye"}
            label={shown ? t.identity.hide(label) : t.identity.show(label)}
            onClick={toggle}
          />
          {copyButton}
        </>
      }
    >
      <span className="secret" data-revealed={shown}>
        {shown ? (
          <span className="mono selectable revealed">{value}</span>
        ) : (
          <span className="mono masked" aria-label={t.identity.hidden(label)}>
            ••••••••••
          </span>
        )}
      </span>
    </Field>
  );
}

/**
 * The account's one Identity, read-only. Opening it is the explicit action
 * that decrypts its values (as with a secure note); they are dropped when
 * this unmounts, including on lock. Copies go through Rust.
 */
export function IdentityDetail({ item, readOnly, onTag, onEdit }: Props) {
  const toast = useToast();
  const { t, dateLocale } = useI18n();
  const [view, setView] = useState<IdentityView | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api.revealIdentity(item.id).then(
      (v) => !cancelled && setView(v),
      (e) => {
        if (cancelled) return;
        setFailed(true);
        toast(errorMessage(e, t, t.identity.loadFailed), "error");
      },
    );
    return () => {
      cancelled = true;
      setView(null);
    };
    // `t` is left out: a language change must not decrypt the identity again.
  }, [item.id, toast]);

  const copy = (field: IdentityCopyField, label: string) => async () => {
    try {
      const r = await api.copyIdentityField(item.id, field);
      toast(t.identity.copied(label, r.clearAfterSeconds));
      return true;
    } catch (e) {
      toast(errorMessage(e, t, t.common.couldNotCopy), "error");
      return false;
    }
  };

  const row = (def: IdentityFieldDef, value: string) => {
    const label = t.identity.fields[def.key];
    const copyButton = <CopyButton label={t.identity.copy(label)} onCopy={copy(def.key, label)} />;
    if (def.kind === "masked") {
      return <MaskedField key={def.key} label={label} value={value} copyButton={copyButton} />;
    }
    return (
      <Field key={def.key} label={label} actions={copyButton}>
        <span className={`selectable${def.kind === "tel" ? " mono" : ""}`} data-truncate="">
          {def.kind === "date" ? formatBirthDate(value, dateLocale) : value}
        </span>
      </Field>
    );
  };

  const fields = view?.fields;
  const custom = fields?.custom ?? [];

  return (
    <article className="item">
      <header className="item-head" data-tauri-drag-region>
        <span className="avatar avatar-lg avatar-identity" aria-hidden="true">
          <Icon name="idCard" size={28} />
        </span>
        <div className="item-head-text">
          <h2 className="item-title">{identityTitle(item.title, t.identity.title)}</h2>
          {item.title.trim() && <p className="item-kind">{t.identity.kind}</p>}
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

      {!fields ? (
        !failed && <p className="muted">{t.common.decrypting}</p>
      ) : isEmptyIdentity(fields) ? (
        <div className="identity-empty">
          <Icon name="idCard" size={26} />
          <p className="identity-empty-title">{t.identity.emptyTitle}</p>
          <p className="muted">{t.identity.emptyBody}</p>
          <button className="btn btn-primary" onClick={onEdit} disabled={readOnly}>
            {t.identity.fillIn}
          </button>
        </div>
      ) : (
        <>
          {visibleSections(fields).map((section) => (
            <section key={section.id} aria-label={t.identity.sections[section.id]}>
              <h3 className="group-title">{t.identity.sections[section.id]}</h3>
              <div className="group">
                {section.id === "address" && view?.address && (
                  <Field
                    label={t.identity.fullAddress}
                    actions={<CopyButton label={t.identity.copyAddress} onCopy={copy("address", t.identity.fullAddress)} />}
                  >
                    <p className="address-block selectable">{view.address}</p>
                  </Field>
                )}
                {section.fields.map((def) => row(def, fields[def.key] ?? ""))}
              </div>
            </section>
          ))}

          {custom.length > 0 && (
            <section aria-label={t.identity.sections.custom}>
              <h3 className="group-title">{t.identity.sections.custom}</h3>
              <div className="group">
                {custom.map((c, i) => {
                  const copyButton = (
                    <CopyButton label={t.identity.copy(c.label)} onCopy={copy(`custom:${i}`, c.label)} />
                  );
                  return c.hidden ? (
                    <MaskedField key={`${i}-${c.label}`} label={c.label} value={c.value} copyButton={copyButton} />
                  ) : (
                    <Field key={`${i}-${c.label}`} label={c.label} actions={copyButton}>
                      <span className="selectable note-body">{c.value}</span>
                    </Field>
                  );
                })}
              </div>
            </section>
          )}

          {fields.notes && (
            <section aria-label={t.identity.sections.notes}>
              <h3 className="group-title">{t.identity.sections.notes}</h3>
              <div className="group">
                <div className="row">
                  <p className="note-body selectable">{fields.notes}</p>
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
      </footer>
      <p className="group-note">{t.identity.readOnly}</p>
    </article>
  );
}

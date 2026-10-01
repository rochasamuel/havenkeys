import { Fragment, useCallback, useEffect, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import { formatDay } from "../lib/format";
import { useRevealedSecret } from "../lib/hooks";
import type { FieldView, SectionView } from "../lib/types";
import { CopyButton } from "../components/CopyButton";
import { Field, IconButton, TotpField } from "../components/Field";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

/**
 * A login's custom fields, under its built-in ones (spec
 * 2026-09-30-login-custom-fields §5.2). Loaded when the login opens or
 * changes; Password values only on the eye, copies done in Rust.
 */
export function CustomSections({ itemId, updatedAt }: { itemId: string; updatedAt: number }) {
  const { t } = useI18n();
  const [sections, setSections] = useState<SectionView[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let live = true;
    setFailed(false);
    api.loginFields(itemId).then(
      (s) => live && setSections(s),
      () => live && setFailed(true),
    );
    return () => {
      live = false;
      setSections(null);
    };
  }, [itemId, updatedAt]);

  if (failed) return <p className="group-note cf-load-failed">{t.fields.loadFailed}</p>;
  if (!sections) return null;
  return (
    <>
      {sections.map((s) => (
        <Fragment key={s.id}>
          {s.title && <h3 className="group-title">{s.title}</h3>}
          <div className="group">
            {s.fields.map((f) => (
              <FieldRow key={f.id} itemId={itemId} field={f} />
            ))}
          </div>
        </Fragment>
      ))}
    </>
  );
}

function Empty({ label }: { label: string }) {
  return (
    <Field label={label}>
      <span className="muted">—</span>
    </Field>
  );
}

function FieldRow({ itemId, field }: { itemId: string; field: FieldView }) {
  const { t, dateLocale } = useI18n();
  const toast = useToast();
  const name = field.label || t.fields.types[field.type];
  const copy = useCallback(async () => {
    try {
      const r = await api.copyLoginField(itemId, field.id);
      toast(t.fields.copied(name, r.clearAfterSeconds));
      return true;
    } catch (e) {
      toast(errorMessage(e, t, t.common.couldNotCopy), "error");
      return false;
    }
  }, [itemId, field.id, name, toast, t]);
  const copyButton = <CopyButton label={t.fields.copy(name)} onCopy={copy} />;

  switch (field.type) {
    case "password":
      return field.hasValue ? (
        <PasswordRow itemId={itemId} fieldId={field.id} label={field.label} name={name} copyButton={copyButton} />
      ) : (
        <Empty label={field.label} />
      );
    case "otp":
      return field.hasOtp ? (
        <TotpField
          label={field.label}
          load={() => api.loginFieldTotp(itemId, field.id)}
          loadKey={`${itemId}/${field.id}`}
          actions={copyButton}
        />
      ) : (
        <Empty label={field.label} />
      );
    case "address":
      return field.formatted ? (
        <Field label={field.label} actions={copyButton}>
          <span className="selectable multiline">{field.formatted}</span>
        </Field>
      ) : (
        <Empty label={field.label} />
      );
    case "url":
      return field.value ? (
        <Field label={field.label} actions={copyButton}>
          <div className="url-value">
            <button
              type="button"
              className="url-link"
              title={t.detail.openWebsite}
              onClick={() =>
                void api
                  .openLoginFieldUrl(itemId, field.id)
                  .catch((e) => toast(errorMessage(e, t, t.detail.openWebsiteFailed), "error"))
              }
            >
              {field.value}
            </button>
          </div>
        </Field>
      ) : (
        <Empty label={field.label} />
      );
    default:
      return field.value ? (
        <Field label={field.label} actions={copyButton}>
          <span className="selectable multiline">
            {field.type === "date" ? formatDay(field.value, dateLocale) : field.value}
          </span>
        </Field>
      ) : (
        <Empty label={field.label} />
      );
  }
}

/** Dots until the eye; the value is dropped on hide, after a while, and on unmount (lock, other item). */
function PasswordRow({
  itemId,
  fieldId,
  label,
  name,
  copyButton,
}: {
  itemId: string;
  fieldId: string;
  label: string;
  name: string;
  copyButton: ReactNode;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const secret = useRevealedSecret(useCallback(() => api.revealLoginField(itemId, fieldId), [itemId, fieldId]));
  return (
    <Field
      label={label}
      actions={
        <>
          <IconButton
            icon={secret.value === null ? "eye" : "eyeOff"}
            label={secret.value === null ? t.fields.show(name) : t.fields.hide(name)}
            onClick={() =>
              secret.value === null
                ? void secret.reveal().catch((e) => toast(errorMessage(e, t, t.common.couldNotReveal), "error"))
                : secret.hide()
            }
          />
          {copyButton}
        </>
      }
    >
      <span className="secret" data-revealed={secret.value !== null}>
        {secret.value === null ? (
          <span className="mono masked" aria-label={t.common.hiddenPassword}>
            ••••••••••••
          </span>
        ) : (
          <span className="mono selectable revealed">{secret.value}</span>
        )}
      </span>
    </Field>
  );
}

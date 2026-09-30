import { useEffect, useMemo, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import type { CardBrand, CardInput, CardNumberCheck, ItemOverview } from "../lib/types";
import { BRAND_NAMES, CARD_BRAND_CHOICES, digitsOnly, formatExpiry, groupNumber, parseExpiryInput } from "../lib/card";
import { CardBrandLogo } from "../components/CardBrandLogo";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

interface Props {
  /** Absent for a new card. */
  existing?: ItemOverview;
  /** Offline: saving would fail, so the control is disabled up front. */
  readOnly?: boolean;
  onCancel: () => void;
  onSaved: (item: ItemOverview) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

interface Draft {
  title: string;
  cardholderName: string;
  brand: CardBrand | "";
  number: string;
  verificationNumber: string;
  expiry: string;
  notes: string;
}

const EMPTY: Draft = { title: "", cardholderName: "", brand: "", number: "", verificationNumber: "", expiry: "", notes: "" };

/**
 * Create or edit a card. The saved number and code are never loaded: left
 * empty they are kept, typed they replace the saved ones. Rust validates.
 */
export function CardEditor({ existing, readOnly, onCancel, onSaved, onDirtyChange }: Props) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<Draft | null>(existing ? null : EMPTY);
  const [initial, setInitial] = useState<string>(JSON.stringify(EMPTY));
  const [check, setCheck] = useState<CardNumberCheck | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!existing) return;
    let cancelled = false;
    api.revealCard(existing.id).then(
      (v) => {
        if (cancelled) return;
        const d: Draft = {
          ...EMPTY,
          title: existing.title,
          cardholderName: v.cardholderName ?? "",
          brand: v.brand ?? "",
          expiry: v.expiry ? formatExpiry(v.expiry) : "",
          notes: v.notes ?? "",
        };
        setDraft(d);
        setInitial(JSON.stringify(d));
      },
      (e) => !cancelled && setError(errorMessage(e, t, t.card.loadFailed)),
    );
    return () => {
      cancelled = true;
      setDraft(null);
    };
    // `t` is left out: a language change must not decrypt the card again.
  }, [existing?.id]);

  const digits = digitsOnly(draft?.number ?? "");
  useEffect(() => {
    if (!digits) {
      setCheck(null);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      api.checkCardNumber(digits).then(
        (c) => !cancelled && setCheck(c),
        () => !cancelled && setCheck(null),
      );
    }, 300);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [digits]);

  const dirty = draft !== null && JSON.stringify(draft) !== initial;
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);

  const set = (key: keyof Draft, value: string) => setDraft((d) => (d ? { ...d, [key]: value } : d));
  const shownBrand: CardBrand = draft?.brand || check?.brand || existing?.card?.brand || "other";
  const titlePlaceholder = shownBrand === "other" ? t.card.kind : BRAND_NAMES[shownBrand];
  const showWarning = check !== null && digits.length >= 12 && !check.checkDigitOk;
  const f = t.card.fields;

  const display = useMemo(() => groupNumber(digits, draft?.brand || check?.brand || null), [digits, draft?.brand, check?.brand]);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!draft || saving || readOnly) return;
    const expiry = parseExpiryInput(draft.expiry);
    if (!expiry.ok) {
      setError(t.card.expiryInvalid);
      return;
    }
    const card: CardInput = {
      cardholderName: draft.cardholderName.trim() || null,
      brand: draft.brand || null,
      number: digits ? { op: "set", value: digits } : { op: "keep" },
      verificationNumber: draft.verificationNumber.trim() ? { op: "set", value: draft.verificationNumber.trim() } : { op: "keep" },
      expiry: expiry.value,
      notes: draft.notes.trim() ? draft.notes : null,
    };
    setSaving(true);
    setError(null);
    try {
      const input = { itemType: "card" as const, title: draft.title, card };
      onSaved(existing ? await api.updateItem(existing.id, input) : await api.createItem(input));
    } catch (err) {
      setError(errorMessage(err, t, t.editor.saveFailed));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form className="editor" onSubmit={submit}>
      <header className="editor-head" data-tauri-drag-region>
        <h2>{existing ? t.card.edit : t.card.newTitle}</h2>
        <div className="editor-actions">
          <button type="button" className="btn btn-small" onClick={onCancel} disabled={saving}>
            {t.common.cancel}
          </button>
          <button type="submit" className="btn btn-small btn-primary" disabled={!draft || saving || readOnly}>
            {saving ? t.common.saving : t.common.save}
          </button>
        </div>
      </header>

      {!draft ? (
        !error && <p className="muted">{t.common.decrypting}</p>
      ) : (
        <div className="group">
          <label className="row edit-row">
            <span className="edit-label">{f.title}</span>
            <input className="edit-input" value={draft.title} placeholder={titlePlaceholder} maxLength={256}
              onChange={(e) => set("title", e.target.value)} autoComplete="off" autoFocus />
          </label>
          <label className="row edit-row">
            <span className="edit-label">{f.cardholderName}</span>
            <input className="edit-input" value={draft.cardholderName} maxLength={256}
              onChange={(e) => set("cardholderName", e.target.value)} autoComplete="off" spellCheck={false} />
          </label>
          <label className="row edit-row">
            <span className="edit-label">{f.number}</span>
            <span className="card-number-input">
              <input className="edit-input mono" inputMode="numeric" value={display} maxLength={23}
                placeholder={existing ? t.card.keepNumber : undefined}
                onChange={(e) => set("number", e.target.value)} autoComplete="off" spellCheck={false} />
              {digits && <CardBrandLogo brand={shownBrand} width={30} />}
            </span>
          </label>
          {showWarning && (
            <p className="field-warning" role="status">
              {t.card.checkDigitWarning}
            </p>
          )}
          <label className="row edit-row">
            <span className="edit-label">{f.brand}</span>
            <select className="edit-input" value={draft.brand} onChange={(e) => set("brand", e.target.value)}>
              <option value="">{t.card.detectBrand}</option>
              {CARD_BRAND_CHOICES.map((b) => (
                <option key={b} value={b}>
                  {BRAND_NAMES[b]}
                </option>
              ))}
              <option value="other">{t.card.otherBrand}</option>
            </select>
          </label>
          <label className="row edit-row">
            <span className="edit-label">{f.verificationNumber}</span>
            <input className="edit-input mono" type="password" inputMode="numeric" value={draft.verificationNumber}
              maxLength={8} placeholder={existing ? t.card.keepVerificationNumber : undefined}
              onChange={(e) => set("verificationNumber", e.target.value)} autoComplete="off" />
          </label>
          <label className="row edit-row">
            <span className="edit-label">{f.expiry}</span>
            <input className="edit-input mono" value={draft.expiry} placeholder={t.card.expiryPlaceholder} maxLength={9}
              inputMode="numeric" onChange={(e) => set("expiry", e.target.value)} autoComplete="off" />
          </label>
          <textarea className="edit-area" value={draft.notes} aria-label={f.notes} placeholder={f.notes}
            onChange={(e) => set("notes", e.target.value)} spellCheck={false} rows={4} />
        </div>
      )}

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {readOnly && (
        <p className="form-error" role="status">
          {t.common.offlineReadOnly}
        </p>
      )}
    </form>
  );
}

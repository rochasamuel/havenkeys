import { useEffect, useRef, useState, type ClipboardEvent, type FormEvent } from "react";
import { api } from "../lib/api";
import type { BankAccountType, BankInput, ItemOverview, SecretUpdate, UrlRule } from "../lib/types";
import { splitPasted } from "../lib/bank";
import { fromViews, sectionsInput, type EditSection } from "../lib/customFields";
import { EMPTY, KEEP, toUpdate, type SecretEdit } from "../lib/secretEdit";
import { TagsEditor, type TagsEditorHandle } from "../components/TagsEditor";
import { FileRows, useFileEdits } from "../components/FileRows";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { CustomFieldsEditor } from "./CustomFieldsEditor";
import { PasswordEdit, WebsitesEdit } from "./LoginParts";

interface Props {
  /** Absent for a new bank account. */
  existing?: ItemOverview;
  /** Offline: saving would fail, so the control is disabled up front. */
  readOnly?: boolean;
  onCancel: () => void;
  onSaved: (item: ItemOverview) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

interface Draft {
  title: string;
  bank: string;
  holderName: string;
  accountType: BankAccountType | "";
  branch: string;
  branchDigit: string;
  /** Typed only: a saved account is never loaded, and left empty it is kept. */
  account: string;
  accountDigit: string;
  clearAccount: boolean;
  password: SecretEdit;
  urls: UrlRule[];
  notes: string;
  tags: string[];
  /** null while an existing account's fields load: the save then keeps them. */
  sections: EditSection[] | null;
}

const ACCOUNT_TYPES: BankAccountType[] = ["checking", "savings", "payment", "other"];

const EMPTY_DRAFT: Draft = {
  title: "", bank: "", holderName: "", accountType: "", branch: "", branchDigit: "", account: "", accountDigit: "",
  clearAccount: false, password: EMPTY, urls: [{ url: "", matchType: "domain" }], notes: "", tags: [], sections: [],
};

const text = (s: string) => s.trim() || null;
const typed = (s: string): SecretUpdate | null => (s.trim() ? { op: "set", value: s.trim() } : null);

/** The account and its digit: a new account replaces both; an empty one keeps what is saved. */
function accountUpdates(d: Draft): Pick<BankInput, "account" | "accountDigit"> {
  if (d.clearAccount) return { account: { op: "clear" }, accountDigit: { op: "clear" } };
  const account = typed(d.account);
  if (account) return { account, accountDigit: typed(d.accountDigit) ?? { op: "clear" } };
  return { account: { op: "keep" }, accountDigit: typed(d.accountDigit) ?? { op: "keep" } };
}

/**
 * Create or edit a bank account. The saved account, its digit and the
 * password are never loaded: left alone they are kept. Rust validates and
 * cleans every value.
 */
export function BankEditor({ existing, readOnly, onCancel, onSaved, onDirtyChange }: Props) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<Draft | null>(existing ? null : EMPTY_DRAFT);
  const [initial, setInitial] = useState(JSON.stringify(EMPTY_DRAFT));
  /** What the saved account already has: kept values and the notes to compare against. */
  const [saved, setSaved] = useState({ account: false, password: false, notes: "" });
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const tagsRow = useRef<TagsEditorHandle>(null);
  const toast = useToast();
  const sideError = (message: string) => toast(message, "error");
  const files = useFileEdits(existing?.attachments, (e) => sideError(errorMessage(e, t, t.attachments.attachFailed)));

  useEffect(() => {
    if (!existing) return;
    let cancelled = false;
    api.revealBank(existing.id).then(
      (v) => {
        if (cancelled) return;
        setSaved({ account: v.hasAccount, password: v.hasPassword, notes: v.notes ?? "" });
        const d: Draft = {
          ...EMPTY_DRAFT,
          title: existing.title,
          bank: v.bank ?? "",
          holderName: v.holderName ?? "",
          accountType: v.accountType ?? "",
          branch: v.branch ?? "",
          branchDigit: v.branchDigit ?? "",
          password: v.hasPassword ? KEEP : EMPTY,
          urls: existing.urls.length ? existing.urls.map((u) => ({ ...u })) : EMPTY_DRAFT.urls,
          notes: v.notes ?? "",
          tags: existing.tags,
          sections: fromViews(v.sections),
        };
        setDraft(d);
        setInitial(JSON.stringify(d));
      },
      (e) => !cancelled && setError(errorMessage(e, t, t.bank.loadFailed)),
    );
    return () => {
      cancelled = true;
      setDraft(null);
    };
    // `t` is left out: a language change must not decrypt the account again.
  }, [existing?.id]);

  const dirty = (draft !== null && JSON.stringify(draft) !== initial) || files.dirty;
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);

  const set = <K extends keyof Draft>(key: K, value: Draft[K]) => setDraft((d) => (d ? { ...d, [key]: value } : d));
  /** A pasted `12345-6` fills the number box and its digit box. */
  const pasteSplit = (number: "branch" | "account", digit: "branchDigit" | "accountDigit") => (e: ClipboardEvent<HTMLInputElement>) => {
    const parts = splitPasted(e.clipboardData.getData("text"));
    if (parts.digit === null) return;
    e.preventDefault();
    setDraft((d) => (d ? { ...d, [number]: parts.number, [digit]: parts.digit } : d));
  };
  const f = t.bank.fields;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!draft || saving || readOnly) return;
    // A tag still being typed is saved too; one Rust would refuse keeps the editor open.
    const tags = tagsRow.current ? tagsRow.current.commit() : draft.tags;
    if (tags === null) return;
    const bank: BankInput = {
      bank: text(draft.bank),
      holderName: text(draft.holderName),
      accountType: draft.accountType || null,
      branch: text(draft.branch),
      branchDigit: text(draft.branchDigit),
      ...accountUpdates(draft),
    };
    setSaving(true);
    setError(null);
    const edit = files.take();
    try {
      const input = {
        itemType: "bank_account" as const,
        title: draft.title,
        tags,
        bank,
        urls: draft.urls.filter((u) => u.url.trim()),
        password: toUpdate(draft.password),
        notes: draft.notes !== saved.notes ? toUpdate({ mode: "set", value: draft.notes }) : { op: "keep" as const },
        sections: sectionsInput(draft.sections, t.fields.types),
      };
      onSaved(existing ? await api.updateItem(existing.id, input, edit) : await api.createItem(input, edit));
    } catch (err) {
      setError(files.saveFailed(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form className="editor" onSubmit={submit}>
      <header className="editor-head" data-tauri-drag-region>
        <h2>{existing ? t.bank.edit : t.bank.newTitle}</h2>
        <div className="editor-actions">
          <button
            type="button"
            className="btn btn-small"
            onClick={() => {
              files.discard();
              onCancel();
            }}
            disabled={saving}
          >
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
        <>
          <div className="group">
            <label className="row edit-row">
              <span className="edit-label">{f.title}</span>
              <input className="edit-input" value={draft.title} placeholder={draft.bank.trim() || t.bank.kind} maxLength={256}
                onChange={(e) => set("title", e.target.value)} autoComplete="off" autoFocus />
            </label>
          </div>
          <div className="group">
            <TagsEditor
              value={draft.tags}
              onChange={(next) => set("tags", next)}
              disabled={readOnly || saving}
              handle={tagsRow}
              itemId={existing?.id}
            />
          </div>
          <div className="group">
            <label className="row edit-row">
              <span className="edit-label">{f.bank}</span>
              <input className="edit-input" value={draft.bank} maxLength={64} aria-label={f.bank}
                onChange={(e) => set("bank", e.target.value)} autoComplete="off" spellCheck={false} />
            </label>
            <label className="row edit-row">
              <span className="edit-label">{f.holderName}</span>
              <input className="edit-input" value={draft.holderName} maxLength={256} aria-label={f.holderName}
                onChange={(e) => set("holderName", e.target.value)} autoComplete="off" spellCheck={false} />
            </label>
            <label className="row edit-row">
              <span className="edit-label">{f.accountType}</span>
              <select className="edit-input" value={draft.accountType} aria-label={f.accountType}
                onChange={(e) => set("accountType", e.target.value as Draft["accountType"])}>
                <option value="">{t.bank.noType}</option>
                {ACCOUNT_TYPES.map((type) => (
                  <option key={type} value={type}>
                    {t.bank.types[type]}
                  </option>
                ))}
              </select>
            </label>
            <div className="row edit-row">
              <span className="edit-label">{f.branch}</span>
              <span className="bank-number">
                <input className="edit-input mono" inputMode="numeric" value={draft.branch} maxLength={8} aria-label={f.branch}
                  onChange={(e) => set("branch", e.target.value)} onPaste={pasteSplit("branch", "branchDigit")}
                  autoComplete="off" spellCheck={false} />
                <span className="bank-dash" aria-hidden="true">–</span>
                <input className="edit-input mono bank-digit" value={draft.branchDigit} maxLength={1} placeholder={t.bank.digit}
                  aria-label={t.bank.digitOf(f.branch)} onChange={(e) => set("branchDigit", e.target.value.toUpperCase())}
                  autoComplete="off" spellCheck={false} />
              </span>
            </div>
            <div className="row edit-row">
              <span className="edit-label">{f.account}</span>
              {draft.clearAccount ? (
                <div className="edit-secret">
                  <span className="muted">{t.bank.accountRemoved}</span>
                  <span className="edit-secret-actions">
                    <button type="button" className="btn btn-small" aria-label={`${t.common.undo}: ${f.account}`}
                      onClick={() => set("clearAccount", false)}>
                      {t.common.undo}
                    </button>
                  </span>
                </div>
              ) : (
                <span className="bank-number">
                  <input className="edit-input mono" inputMode="numeric" value={draft.account} maxLength={20} aria-label={f.account}
                    placeholder={saved.account ? t.bank.keepAccount : undefined}
                    onChange={(e) => set("account", e.target.value)} onPaste={pasteSplit("account", "accountDigit")}
                    autoComplete="off" spellCheck={false} />
                  <span className="bank-dash" aria-hidden="true">–</span>
                  <input className="edit-input mono bank-digit" value={draft.accountDigit} maxLength={2} placeholder={t.bank.digit}
                    aria-label={t.bank.digitOf(f.account)} onChange={(e) => set("accountDigit", e.target.value.toUpperCase())}
                    autoComplete="off" spellCheck={false} />
                  {saved.account && !draft.account && !draft.accountDigit && (
                    <button type="button" className="btn btn-small btn-quiet-danger" aria-label={`${t.common.remove}: ${f.account}`}
                      onClick={() => set("clearAccount", true)}>
                      {t.common.remove}
                    </button>
                  )}
                </span>
              )}
            </div>
            <PasswordEdit value={draft.password} onChange={(next) => set("password", next)}
              saved={saved.password} label={f.password} />
          </div>

          <WebsitesEdit urls={draft.urls} onChange={(next) => set("urls", next)} />

          <h3 className="group-title">{f.notes}</h3>
          <div className="group">
            <textarea className="edit-area" value={draft.notes} aria-label={f.notes} placeholder={f.notes}
              onChange={(e) => set("notes", e.target.value)} spellCheck={false} rows={4} />
          </div>

          {draft.sections !== null && (
            <CustomFieldsEditor
              itemId={existing?.id}
              sections={draft.sections}
              onChange={(next) => set("sections", next)}
              disabled={saving || !!readOnly}
              onError={sideError}
              onAttach={() => void files.attach()}
              attaching={files.picking}
            />
          )}
          <FileRows files={files.rows} disabled={saving || !!readOnly} onRemove={files.remove} />
        </>
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

import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import type { CustomField, IdentityFields, ItemOverview } from "../lib/types";
import { IDENTITY_SECTIONS, type IdentityFieldDef } from "../lib/identity";
import { Icon } from "../components/Icon";
import { TagsEditor, type TagsEditorHandle } from "../components/TagsEditor";
import { Switch } from "../components/Switch";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

interface Props {
  existing: ItemOverview;
  /** Offline: saving would fail, so the control is disabled up front. */
  readOnly?: boolean;
  onCancel: () => void;
  onSaved: (item: ItemOverview) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

const MAX_CUSTOM = 50;

const inputType = (def: IdentityFieldDef) =>
  def.kind === "date" ? "date" : def.kind === "tel" ? "tel" : def.kind === "email" ? "email" : def.kind === "url" ? "url" : "text";

/**
 * Edit the account's one Identity. The values are decrypted when the editor
 * opens and the whole identity is sent back on save; Rust validates it.
 */
export function IdentityEditor({ existing, readOnly, onCancel, onSaved, onDirtyChange }: Props) {
  const { t } = useI18n();
  const [fields, setFields] = useState<IdentityFields | null>(null);
  const [tags, setTags] = useState<string[]>(existing.tags);
  const tagsRow = useRef<TagsEditorHandle>(null);
  const [initial, setInitial] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api.revealIdentity(existing.id).then(
      (v) => {
        if (cancelled) return;
        setFields(v.fields);
        setInitial(JSON.stringify({ fields: v.fields, tags: existing.tags }));
      },
      (e) => !cancelled && setError(errorMessage(e, t, t.identity.loadFailed)),
    );
    return () => {
      cancelled = true;
      setFields(null);
    };
    // `t` is left out: a language change must not decrypt the identity again.
  }, [existing.id]);

  const dirty = fields !== null && JSON.stringify({ fields, tags }) !== initial;
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);

  const set = (key: keyof IdentityFields, value: string) => setFields((f) => (f ? { ...f, [key]: value } : f));
  const custom = fields?.custom ?? [];
  const setCustom = (list: CustomField[]) => setFields((f) => (f ? { ...f, custom: list } : f));
  const editCustom = (i: number, change: Partial<CustomField>) =>
    setCustom(custom.map((c, j) => (j === i ? { ...c, ...change } : c)));

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!fields || saving || readOnly) return;
    // A tag still being typed is saved too; one Rust would refuse keeps the editor open.
    const finalTags = tagsRow.current ? tagsRow.current.commit() : tags;
    if (finalTags === null) return;
    setSaving(true);
    setError(null);
    try {
      const saved = await api.updateItem(existing.id, { itemType: "identity", title: "", identity: fields, tags: finalTags });
      onSaved(saved);
    } catch (err) {
      setError(errorMessage(err, t, t.editor.saveFailed));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form className="editor" onSubmit={submit}>
      <header className="editor-head" data-tauri-drag-region>
        <h2>{t.identity.edit}</h2>
        <div className="editor-actions">
          <button type="button" className="btn btn-small" onClick={onCancel} disabled={saving}>
            {t.common.cancel}
          </button>
          <button type="submit" className="btn btn-small btn-primary" disabled={!fields || saving || readOnly}>
            {saving ? t.common.saving : t.common.save}
          </button>
        </div>
      </header>

      {!fields ? (
        !error && <p className="muted">{t.common.decrypting}</p>
      ) : (
        <>
          <div className="group">
            <TagsEditor value={tags} onChange={setTags} disabled={readOnly || saving} handle={tagsRow} />
          </div>

          {IDENTITY_SECTIONS.map((section, s) => (
            <section key={section.id} aria-label={t.identity.sections[section.id]}>
              <h3 className="group-title">{t.identity.sections[section.id]}</h3>
              <div className="group">
                {section.fields.map((def, i) => (
                  <label className="row edit-row" key={def.key}>
                    <span className="edit-label">{t.identity.fields[def.key]}</span>
                    <input
                      className={`edit-input${def.kind === "masked" || def.kind === "tel" ? " mono" : ""}`}
                      type={inputType(def)}
                      value={fields[def.key] ?? ""}
                      onChange={(e) => set(def.key, e.target.value)}
                      maxLength={def.max}
                      autoComplete="off"
                      spellCheck={false}
                      autoFocus={s === 0 && i === 0}
                    />
                  </label>
                ))}
              </div>
            </section>
          ))}

          <h3 className="group-title">{t.identity.sections.custom}</h3>
          <div className="group">
            {custom.map((c, i) => (
              <div className="row edit-row custom-edit" key={i}>
                <input
                  className="edit-input custom-edit-label"
                  value={c.label}
                  onChange={(e) => editCustom(i, { label: e.target.value })}
                  placeholder={t.identity.custom.label}
                  aria-label={t.identity.custom.labelN(i + 1)}
                  maxLength={128}
                  autoComplete="off"
                />
                <input
                  className={`edit-input custom-edit-value${c.hidden ? " mono" : ""}`}
                  type={c.hidden ? "password" : "text"}
                  value={c.value}
                  onChange={(e) => editCustom(i, { value: e.target.value })}
                  placeholder={t.identity.custom.value}
                  aria-label={t.identity.custom.valueN(i + 1)}
                  maxLength={4096}
                  autoComplete="off"
                  spellCheck={false}
                />
                <span className="custom-edit-controls">
                  <span className="custom-edit-hidden">
                    <span className="muted">{t.identity.custom.hidden}</span>
                    <Switch
                      label={t.identity.custom.hiddenN(i + 1)}
                      checked={c.hidden}
                      onChange={(hidden) => editCustom(i, { hidden })}
                    />
                  </span>
                  <button
                    type="button"
                    className="icon-btn"
                    onClick={() => setCustom(custom.filter((_, j) => j !== i))}
                    aria-label={t.identity.custom.remove(i + 1)}
                    title={t.identity.custom.remove(i + 1)}
                  >
                    <Icon name="x" size={16} />
                  </button>
                </span>
              </div>
            ))}
            <button
              type="button"
              className="row row-button add-row"
              onClick={() => setCustom([...custom, { label: "", value: "", hidden: false }])}
              disabled={custom.length >= MAX_CUSTOM}
            >
              <Icon name="plus" size={15} /> {t.identity.custom.add}
            </button>
          </div>

          <h3 className="group-title">{t.identity.sections.notes}</h3>
          <div className="group">
            <textarea
              className="edit-area"
              value={fields.notes ?? ""}
              placeholder={t.identity.notesPlaceholder}
              aria-label={t.identity.sections.notes}
              onChange={(e) => set("notes", e.target.value)}
              spellCheck={false}
              rows={4}
            />
          </div>
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

import { Fragment, useCallback, useEffect, useRef, useState, type DragEvent } from "react";
import { Icon } from "../components/Icon";
import { TotpEdit } from "../components/TotpEdit";
import { api } from "../lib/api";
import {
  addField,
  dropTarget,
  FIELD_TYPES,
  fieldCount,
  MAX_FIELDS,
  MAX_SECTIONS,
  moveField,
  moveSection,
  newField,
  newSection,
  placeField,
  removeField,
  removeSection,
  updateField,
  updateSection,
  type DropTarget,
  type EditField,
  type EditSection,
} from "../lib/customFields";
import { useRevealedSecret } from "../lib/hooks";
import { EMPTY, KEEP } from "../lib/secretEdit";
import type { AddressPart, FieldType } from "../lib/types";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

const ADDRESS_PARTS: AddressPart[] = [
  "street",
  "number",
  "complement",
  "neighborhood",
  "city",
  "state",
  "postalCode",
  "country",
];

/** Only a field's own handle starts this drag; other drags (text) are left alone. */
const DRAG_TYPE = "application/x-havenkeys-field";

function focusHandle(key: string) {
  requestAnimationFrame(() => document.querySelector<HTMLElement>(`[data-cf-handle="${key}"]`)?.focus());
}

/** A login's custom fields in the editor (spec 2026-09-30-login-custom-fields §5.3). */
export function CustomFieldsEditor({
  itemId,
  sections,
  onChange,
  disabled,
  onError,
}: {
  /** The saved login, for revealing a kept Password; absent for a new login. */
  itemId?: string;
  sections: EditSection[];
  onChange: (next: EditSection[]) => void;
  disabled: boolean;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const [menuOpen, setMenuOpen] = useState(false);
  const [confirming, setConfirming] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const dragKey = useRef<string | null>(null);
  // What the drag shows: the field being moved (and its height, for the
  // placeholder) and where it would land.
  const [dragging, setDragging] = useState<{ key: string; height: number } | null>(null);
  const [target, setTarget] = useState<DropTarget | null>(null);
  const full = fieldCount(sections) >= MAX_FIELDS;

  // Close the add menu on Escape or a click anywhere else, as the New menu does.
  useEffect(() => {
    if (!menuOpen) return;
    const onDown = (e: MouseEvent) => {
      if (!menuRef.current?.contains(e.target as Node)) setMenuOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setMenuOpen(false);
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [menuOpen]);

  function add(type: FieldType) {
    setMenuOpen(false);
    onChange(addField(sections, newField(type, t.fields.types[type])));
  }

  function aim(next: DropTarget | null) {
    setTarget((cur) => (cur?.sectionKey === next?.sectionKey && cur?.beforeKey === next?.beforeKey ? cur : next));
  }

  /** Accept the drag here; false for anything that is not a field's handle. */
  function accept(e: DragEvent): boolean {
    if (!dragKey.current) return false;
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = "move";
    return true;
  }

  function startDrag(key: string, height: number) {
    dragKey.current = key;
    // After the browser has taken the drag image: changing the dragged row
    // inside dragstart can cancel the drag.
    setTimeout(() => setDragging({ key, height }), 0);
  }

  function endDrag() {
    dragKey.current = null;
    setDragging(null);
    setTarget(null);
  }

  function overRow(e: DragEvent, sectionKey: string, overKey: string) {
    if (!accept(e)) return;
    const box = e.currentTarget.getBoundingClientRect();
    aim(dropTarget(sections, dragKey.current!, sectionKey, overKey, e.clientY > box.top + box.height / 2));
  }

  function overSection(e: DragEvent, sectionKey: string) {
    if (!accept(e)) return;
    aim(dropTarget(sections, dragKey.current!, sectionKey, null, false));
  }

  function leaveSection(e: DragEvent, sectionKey: string) {
    if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
    setTarget((cur) => (cur?.sectionKey === sectionKey ? null : cur));
  }

  function drop(e: DragEvent) {
    const key = dragKey.current;
    if (!key) return;
    e.preventDefault();
    e.stopPropagation();
    if (target) onChange(placeField(sections, key, target.sectionKey, target.beforeKey));
    endDrag();
  }

  function move(key: string, delta: -1 | 1) {
    onChange(moveField(sections, key, delta));
    // The row may now sit in another section's list, so React remounts it.
    focusHandle(key);
  }

  return (
    <>
      {sections.map((s, si) => (
        <div key={s.key} className="cf-section">
          <div className="cf-section-head">
            <input
              className="edit-input group-title-input"
              value={s.title}
              onChange={(e) => onChange(updateSection(sections, s.key, e.target.value))}
              placeholder={t.fields.untitled}
              aria-label={t.fields.sectionTitle}
              maxLength={256}
              disabled={disabled}
            />
            <button
              type="button"
              className="icon-btn"
              onClick={() => onChange(moveSection(sections, s.key, -1))}
              disabled={disabled || si === 0}
              aria-label={t.fields.moveSectionUp}
              title={t.fields.moveSectionUp}
            >
              <Icon name="chevronUp" size={16} />
            </button>
            <button
              type="button"
              className="icon-btn"
              onClick={() => onChange(moveSection(sections, s.key, 1))}
              disabled={disabled || si === sections.length - 1}
              aria-label={t.fields.moveSectionDown}
              title={t.fields.moveSectionDown}
            >
              <Icon name="chevronDown" size={16} />
            </button>
            <button
              type="button"
              className="icon-btn"
              onClick={() => (s.fields.length === 0 ? onChange(removeSection(sections, s.key)) : setConfirming(s.key))}
              disabled={disabled}
              aria-label={t.fields.removeSection}
              title={t.fields.removeSection}
            >
              <Icon name="x" size={16} />
            </button>
          </div>
          {confirming === s.key && s.fields.length > 0 && (
            <div className="confirm cf-confirm" role="alert">
              <span>{t.fields.confirmRemoveSection(s.fields.length)}</span>
              <button type="button" className="btn btn-small" onClick={() => setConfirming(null)}>
                {t.common.keep}
              </button>
              <button
                type="button"
                className="btn btn-small btn-danger"
                onClick={() => {
                  setConfirming(null);
                  onChange(removeSection(sections, s.key));
                }}
                disabled={disabled}
              >
                {t.common.remove}
              </button>
            </div>
          )}
          <div
            className="group cf-fields"
            onDragOver={(e) => overSection(e, s.key)}
            onDragLeave={(e) => leaveSection(e, s.key)}
            onDrop={drop}
          >
            {s.fields.length === 0 && target?.sectionKey !== s.key && (
              <p className="row cf-empty muted">{t.fields.emptySection}</p>
            )}
            {s.fields.map((f) => (
              <Fragment key={f.key}>
                {dragging && target?.sectionKey === s.key && target.beforeKey === f.key && (
                  <DropPlaceholder height={dragging.height} onDragOver={accept} onDrop={drop} />
                )}
                <FieldEditRow
                  itemId={itemId}
                  field={f}
                  disabled={disabled}
                  dragging={dragging?.key === f.key}
                  onChange={(change) => onChange(updateField(sections, f.key, change))}
                  onRemove={() => onChange(removeField(sections, f.key))}
                  onMove={(delta) => move(f.key, delta)}
                  onDragStart={(height) => startDrag(f.key, height)}
                  onDragEnd={endDrag}
                  onDragOver={(e) => overRow(e, s.key, f.key)}
                  onDrop={drop}
                  onError={onError}
                />
              </Fragment>
            ))}
            {dragging && target?.sectionKey === s.key && target.beforeKey === null && (
              <DropPlaceholder height={dragging.height} onDragOver={accept} onDrop={drop} />
            )}
          </div>
        </div>
      ))}

      <div className="group cf-add-group">
        <div className="cf-add" ref={menuRef}>
          <button
            type="button"
            className="row row-button add-row"
            onClick={() => setMenuOpen((o) => !o)}
            disabled={disabled || full}
            aria-haspopup="menu"
            aria-expanded={menuOpen}
          >
            <Icon name="plus" size={15} /> {t.fields.addField}
          </button>
          {menuOpen && !disabled && !full && (
            <div className="menu cf-menu" role="menu">
              {FIELD_TYPES.map((type) => (
                <button key={type} type="button" role="menuitem" onClick={() => add(type)}>
                  {t.fields.types[type]}
                </button>
              ))}
            </div>
          )}
        </div>
        <button
          type="button"
          className="row row-button add-row"
          onClick={() => onChange([...sections, newSection()])}
          disabled={disabled || sections.length >= MAX_SECTIONS}
        >
          <Icon name="plus" size={15} /> {t.fields.addSection}
        </button>
      </div>
      {full && <p className="group-note">{t.fields.limit}</p>}
    </>
  );
}

/** Where the dragged field will land: an empty slot of its height. */
function DropPlaceholder({
  height,
  onDragOver,
  onDrop,
}: {
  height: number;
  onDragOver: (e: DragEvent) => void;
  onDrop: (e: DragEvent) => void;
}) {
  return <div className="cf-drop" style={{ height }} onDragOver={onDragOver} onDrop={onDrop} aria-hidden="true" />;
}

function FieldEditRow({
  itemId,
  field,
  disabled,
  dragging,
  onChange,
  onRemove,
  onMove,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDrop,
  onError,
}: {
  itemId?: string;
  field: EditField;
  disabled: boolean;
  /** This row is the one being dragged. */
  dragging: boolean;
  onChange: (change: Partial<EditField>) => void;
  onRemove: () => void;
  onMove: (delta: -1 | 1) => void;
  /** With the row's height, for the placeholder. */
  onDragStart: (height: number) => void;
  onDragEnd: () => void;
  onDragOver: (e: DragEvent) => void;
  onDrop: (e: DragEvent) => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const name = field.label.trim() || t.fields.types[field.type];

  return (
    <div
      className={`row edit-row cf-row${dragging ? " cf-row-dragging" : ""}`}
      onDragOver={onDragOver}
      onDrop={onDrop}
    >
      <button
        type="button"
        className="icon-btn cf-handle"
        data-cf-handle={field.key}
        draggable={!disabled}
        onDragStart={(e) => {
          e.dataTransfer.effectAllowed = "move";
          // Carries no value; the field is found by its key in memory.
          e.dataTransfer.setData(DRAG_TYPE, "");
          // Drag the whole row, not just the handle.
          const row = e.currentTarget.closest<HTMLElement>(".cf-row");
          if (row) e.dataTransfer.setDragImage(row, 16, 16);
          onDragStart(row?.offsetHeight ?? 48);
        }}
        onDragEnd={onDragEnd}
        onKeyDown={(e) => {
          if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
            e.preventDefault();
            onMove(e.key === "ArrowUp" ? -1 : 1);
          }
        }}
        aria-label={t.fields.move(name)}
        title={t.fields.move(name)}
        disabled={disabled}
      >
        <Icon name="grip" size={16} />
      </button>
      <div className="cf-body">
        <input
          className="edit-input cf-label"
          value={field.label}
          onChange={(e) => onChange({ label: e.target.value })}
          placeholder={t.fields.types[field.type]}
          aria-label={t.fields.label}
          maxLength={256}
          disabled={disabled}
        />
        <FieldValueEdit itemId={itemId} field={field} name={name} disabled={disabled} onChange={onChange} onError={onError} />
      </div>
      <button
        type="button"
        className="icon-btn"
        onClick={onRemove}
        disabled={disabled}
        aria-label={t.fields.removeField(name)}
        title={t.fields.removeField(name)}
      >
        <Icon name="x" size={16} />
      </button>
    </div>
  );
}

function FieldValueEdit({
  itemId,
  field,
  name,
  disabled,
  onChange,
  onError,
}: {
  itemId?: string;
  field: EditField;
  name: string;
  disabled: boolean;
  onChange: (change: Partial<EditField>) => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  switch (field.type) {
    case "otp":
      return (
        <TotpEdit
          edit={field.secret}
          onChange={(secret) => onChange({ secret })}
          disabled={disabled}
          ariaLabel={name}
          onError={onError}
          inline
        />
      );
    case "password":
      return <PasswordEdit itemId={itemId} field={field} name={name} disabled={disabled} onChange={onChange} onError={onError} />;
    case "address":
      return (
        <div className="cf-address">
          {ADDRESS_PARTS.map((part) => (
            <input
              key={part}
              className={`edit-input cf-part cf-part-${part}`}
              value={field.address[part] ?? ""}
              onChange={(e) => onChange({ address: { ...field.address, [part]: e.target.value } })}
              placeholder={t.identity.fields[part]}
              aria-label={`${name}: ${t.identity.fields[part]}`}
              maxLength={512}
              disabled={disabled}
            />
          ))}
        </div>
      );
    case "text":
      return (
        <textarea
          className="edit-input cf-text"
          value={field.text}
          onChange={(e) => onChange({ text: e.target.value })}
          aria-label={name}
          rows={Math.min(8, Math.max(1, field.text.split("\n").length))}
          spellCheck={false}
          disabled={disabled}
        />
      );
    default:
      return (
        <input
          className="edit-input"
          type={field.type === "date" ? "date" : field.type === "email" ? "email" : field.type === "phone" ? "tel" : "text"}
          inputMode={field.type === "url" ? "url" : undefined}
          value={field.text}
          onChange={(e) => onChange({ text: e.target.value })}
          placeholder={field.type === "url" ? "https://" : field.type === "email" ? "name@example.com" : undefined}
          aria-label={name}
          maxLength={field.type === "url" ? 2048 : 512}
          spellCheck={false}
          autoCapitalize="off"
          autoComplete="off"
          disabled={disabled}
        />
      );
  }
}

/**
 * A Password field. A saved value stays in Rust (`keep`) and is never loaded
 * to open the editor; a new one is typed or generated here, masked until the eye.
 */
function PasswordEdit({
  itemId,
  field,
  name,
  disabled,
  onChange,
  onError,
}: {
  itemId?: string;
  field: EditField;
  name: string;
  disabled: boolean;
  onChange: (change: Partial<EditField>) => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const [shown, setShown] = useState(false);
  // Opened as `keep`: "Replace" can be taken back. Lost if the row remounts
  // (moved to another section), which only hides that button.
  const [saved] = useState(field.secret.mode === "keep");
  const secret = field.secret;

  async function generate() {
    try {
      const g = await api.generate({
        length: 24,
        uppercase: true,
        lowercase: true,
        digits: true,
        symbols: true,
        avoidAmbiguous: false,
      });
      onChange({ secret: { mode: "set", value: g.password } });
      setShown(true);
    } catch (e) {
      onError(errorMessage(e, t, t.editor.generateFailed));
    }
  }

  if (secret.mode === "keep") {
    return (
      <KeptPassword
        itemId={itemId}
        fieldId={field.id}
        name={name}
        disabled={disabled}
        onReplace={() => onChange({ secret: EMPTY })}
        onRemove={() => onChange({ secret: { mode: "clear" } })}
        onError={onError}
      />
    );
  }
  if (secret.mode === "clear") {
    return (
      <div className="edit-secret">
        <span className="muted">{t.fields.removed}</span>
        <span className="edit-secret-actions">
          <button type="button" className="btn btn-small" onClick={() => onChange({ secret: KEEP })} disabled={disabled}>
            {t.common.undo}
          </button>
        </span>
      </div>
    );
  }
  return (
    <div className="edit-secret">
      <input
        className="edit-input mono"
        type={shown ? "text" : "password"}
        value={secret.mode === "set" ? secret.value : ""}
        onChange={(e) => onChange({ secret: { mode: "set", value: e.target.value } })}
        placeholder={t.common.password}
        autoComplete="new-password"
        spellCheck={false}
        autoCapitalize="off"
        aria-label={name}
        disabled={disabled}
      />
      <span className="edit-secret-actions">
        <button
          type="button"
          className="icon-btn"
          onClick={() => setShown((v) => !v)}
          aria-label={shown ? t.fields.hide(name) : t.fields.show(name)}
          title={shown ? t.fields.hide(name) : t.fields.show(name)}
        >
          <Icon name={shown ? "eyeOff" : "eye"} size={16} />
        </button>
        <button
          type="button"
          className="icon-btn"
          onClick={() => void generate()}
          disabled={disabled}
          aria-label={t.editor.generate}
          title={t.editor.generate}
        >
          <Icon name="dice" size={16} />
        </button>
        {saved && (
          <button type="button" className="btn btn-small btn-quiet" onClick={() => onChange({ secret: KEEP })} disabled={disabled}>
            {t.common.cancel}
          </button>
        )}
      </span>
    </div>
  );
}

/**
 * A saved Password left as it is. The eye shows it, read from Rust, for a
 * moment: display only, never copied into the edit, so the save still
 * sends `keep`. Dropped on hide, on its timer and on unmount (Replace,
 * another item, lock).
 */
function KeptPassword({
  itemId,
  fieldId,
  name,
  disabled,
  onReplace,
  onRemove,
  onError,
}: {
  itemId?: string;
  fieldId?: string;
  name: string;
  disabled: boolean;
  onReplace: () => void;
  onRemove: () => void;
  onError: (message: string) => void;
}) {
  const { t } = useI18n();
  const canReveal = !!itemId && !!fieldId;
  const load = useCallback(
    () => (itemId && fieldId ? api.revealLoginField(itemId, fieldId) : Promise.reject(new Error("no saved field"))),
    [itemId, fieldId],
  );
  const secret = useRevealedSecret(load);
  const label = secret.value === null ? t.fields.show(name) : t.fields.hide(name);
  return (
    <div className="edit-secret">
      {secret.value === null ? (
        <span className="mono masked" aria-label={t.common.hiddenPassword} data-truncate="">
          ••••••••••••
        </span>
      ) : (
        <span className="mono selectable revealed">{secret.value}</span>
      )}
      <span className="edit-secret-actions">
        {canReveal && (
          <button
            type="button"
            className="icon-btn"
            onClick={() =>
              secret.value === null
                ? void secret.reveal().catch((e) => onError(errorMessage(e, t, t.common.couldNotReveal)))
                : secret.hide()
            }
            disabled={disabled}
            aria-label={label}
            title={label}
          >
            <Icon name={secret.value === null ? "eye" : "eyeOff"} size={16} />
          </button>
        )}
        <button type="button" className="btn btn-small" onClick={onReplace} disabled={disabled}>
          {t.editor.replace}
        </button>
        <button type="button" className="btn btn-small btn-quiet-danger" onClick={onRemove} disabled={disabled}>
          {t.common.remove}
        </button>
      </span>
    </div>
  );
}

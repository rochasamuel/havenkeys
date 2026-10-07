import {
  useId,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type KeyboardEvent,
  type Ref,
} from "react";
import { useI18n } from "../i18n/context";
import { MAX_TAG_CHARS, MAX_TAGS, cleanTag, tagProblem } from "../lib/tags";
import { useVaultTags } from "../lib/vaultTags";
import { Icon } from "./Icon";

/** What an editor asks of its tags row when the user saves. */
export interface TagsEditorHandle {
  /**
   * The item's tags with the text still in the field added. Null when Rust
   * would refuse that text: the field keeps it and says why, and the editor
   * must not save.
   */
  commit(): string[] | null;
}

interface Props {
  value: string[];
  onChange: (tags: string[]) => void;
  disabled?: boolean;
  handle?: Ref<TagsEditorHandle>;
}

type Problem = "tooLong" | "notAllowed";

/** One editor row: the item's tags as pills, then a field that suggests the vault's tags. */
export function TagsEditor({ value, onChange, disabled, handle }: Props) {
  const { t } = useI18n();
  const vault = useVaultTags();
  const listId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const [text, setText] = useState("");
  const [active, setActive] = useState(-1); // -1: nothing picked yet, so Enter takes what was typed
  // Why the typed text was not added; cleared as soon as the text changes.
  const [problem, setProblem] = useState<Problem | null>(null);
  const errorId = `${listId}-error`;
  const typed = cleanTag(text);

  const options = useMemo(() => {
    if (!text.trim()) return [];
    const q = text.trim().toLowerCase();
    const found = vault.filter((v) => v.name.includes(q) && !value.includes(v.name)).slice(0, 8);
    const exact = typed !== null && (value.includes(typed) || found.some((f) => f.name === typed));
    return [
      ...found.map((f) => ({ name: f.name, label: f.name, count: f.count as number | null })),
      ...(typed && !exact ? [{ name: typed, label: t.editor.createTag(typed), count: null }] : []),
    ];
  }, [text, typed, vault, value, t]);

  /** The tags with `raw` added, or null (and the reason shown) when Rust would refuse it. */
  const add = (raw: string): string[] | null => {
    const refused = tagProblem(raw);
    if (refused) {
      // Keep the text so it can be corrected, and say why (never echoing it).
      setProblem(refused);
      return null;
    }
    const tag = cleanTag(raw);
    let next = value;
    if (tag && !value.includes(tag) && value.length < MAX_TAGS) {
      next = [...value, tag].sort();
      onChange(next);
    }
    setText("");
    setActive(-1);
    return next;
  };

  useImperativeHandle(
    handle,
    () => ({
      commit: () => {
        if (!text.trim()) return value;
        const next = add(text);
        if (next === null) inputRef.current?.focus();
        return next;
      },
    }),
  );

  const onKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      const picked = e.key === "Enter" ? options[active] : undefined;
      add(picked ? picked.name : text);
    } else if (e.key === "Backspace" && text === "" && value.length) {
      onChange(value.slice(0, -1));
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, options.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => (a <= 0 ? -1 : a - 1));
    } else if (e.key === "Escape" && text) {
      // Only closes the suggestions; an empty field leaves Escape to the editor.
      e.stopPropagation();
      setText("");
    }
  };

  // A paste with commas adds every part but the last and keeps that as text.
  // A part Rust would refuse stays in the field too (commas kept), and the
  // field says why, so nothing is dropped without a word.
  const onType = (e: ChangeEvent<HTMLInputElement>) => {
    const parts = e.target.value.split(",");
    let rest = parts[parts.length - 1] ?? "";
    let why: Problem | null = null;
    if (parts.length > 1) {
      const kept: string[] = [];
      const fresh: string[] = [];
      for (const part of parts.slice(0, -1)) {
        const refused = tagProblem(part);
        if (refused) {
          why ??= refused;
          kept.push(part);
          continue;
        }
        const tag = cleanTag(part);
        if (tag !== null && !value.includes(tag) && !fresh.includes(tag)) fresh.push(tag);
      }
      const room = MAX_TAGS - value.length;
      if (fresh.length) onChange([...value, ...fresh.slice(0, room)].sort());
      if (kept.length) rest = [...kept, rest].join(",");
    }
    setText(rest);
    setActive(-1);
    setProblem(why);
  };

  return (
    <div
      className="row edit-row tags-edit"
      onClick={(e) => {
        if (!(e.target as HTMLElement).closest("button")) inputRef.current?.focus();
      }}
    >
      <span className="edit-label">{t.editor.tags}</span>
      <div className="tags-edit-line">
        {value.map((tag) => (
          <span className="chip tag-chip" key={tag}>
            {tag}
            <button
              type="button"
              className="tag-remove"
              disabled={disabled}
              onClick={() => onChange(value.filter((v) => v !== tag))}
              aria-label={t.editor.removeTag(tag)}
            >
              <Icon name="x" size={12} />
            </button>
          </span>
        ))}
        {value.length >= MAX_TAGS ? (
          <span className="tags-limit">{t.editor.tagLimit}</span>
        ) : (
          <input
            ref={inputRef}
            className="edit-input tags-input"
            value={text}
            disabled={disabled}
            maxLength={MAX_TAG_CHARS + 8 /* spaces collapse; Rust has the final say */}
            placeholder={t.editor.addTag}
            aria-label={t.editor.addTag}
            role="combobox"
            aria-expanded={options.length > 0}
            aria-controls={listId}
            aria-autocomplete="list"
            aria-invalid={problem ? true : undefined}
            aria-describedby={problem ? errorId : undefined}
            aria-activedescendant={active >= 0 && active < options.length ? `${listId}-${active}` : undefined}
            autoComplete="off"
            onChange={onType}
            onKeyDown={onKey}
            onBlur={() => text && add(text)}
          />
        )}
      </div>
      {problem && (
        <p className="tags-error" id={errorId} role="alert">
          {problem === "tooLong" ? t.editor.tagTooLong : t.editor.tagNotAllowed}
        </p>
      )}
      {options.length > 0 && (
        <ul className="tags-suggest" id={listId} role="listbox" aria-label={t.editor.tagSuggestions}>
          {options.map((o, i) => (
            <li
              key={o.label}
              id={`${listId}-${i}`}
              role="option"
              aria-selected={i === active}
              onMouseDown={(e) => {
                e.preventDefault();
                add(o.name);
              }}
            >
              <span>{o.label}</span>
              {o.count !== null && <span className="nav-count">{o.count}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

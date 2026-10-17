import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, ApiError } from "../lib/api";
import { formatSize } from "../lib/format";
import type { AttachmentMeta, FilesEdit, PendingFile } from "../lib/types";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { Icon } from "./Icon";

/** The types Rust sniffs as raster images, the only ones it lets us preview. */
const IMAGE_TYPES = new Set(["image/png", "image/jpeg", "image/webp", "image/gif"]);

export const isImage = (mime: string) => IMAGE_TYPES.has(mime);

/** A file in the editor: on the item (`key` is its id) or picked and waiting for Save (`key` is its token). */
export interface FileRow {
  key: string;
  name: string;
  size: number;
  mime: string;
  pending: boolean;
}

/** The editor's file list; `children` is a trailing row such as "Attach a file". */
export function FileRows({
  files,
  disabled,
  onRemove,
  children,
}: {
  files: FileRow[];
  disabled: boolean;
  onRemove: (row: FileRow) => void;
  children?: ReactNode;
}) {
  const { t, dateLocale } = useI18n();
  if (files.length === 0 && !children) return null;
  return (
    <section aria-label={t.attachments.title}>
      <h3 className="group-title">{t.attachments.title}</h3>
      <ul className="group attachment-list">
        {files.map((f) => (
          <li className="row attachment-row" key={f.key}>
            <Icon name={isImage(f.mime) ? "image" : "file"} size={18} className="attachment-icon" />
            <div className="row-main">
              <div className="row-value attachment-name" title={f.name} data-truncate="">
                {f.name}
              </div>
              <div className="attachment-meta">
                {formatSize(f.size, dateLocale)}
                {f.pending && <span className="attachment-pending"> · {t.attachments.notSavedYet}</span>}
              </div>
            </div>
            <button
              type="button"
              className="icon-btn attachment-remove"
              onClick={() => onRemove(f)}
              disabled={disabled}
              aria-label={t.attachments.remove(f.name)}
              title={t.attachments.remove(f.name)}
            >
              <Icon name="minusCircle" size={18} />
            </button>
          </li>
        ))}
        {children && <li>{children}</li>}
      </ul>
    </section>
  );
}

/** The plain "Attach a file" row, for editors without a custom-fields menu. */
export function AttachRow({ onClick, disabled }: { onClick: () => void; disabled: boolean }) {
  const { t } = useI18n();
  return (
    <button type="button" className="row row-button add-row" onClick={onClick} disabled={disabled}>
      <Icon name="paperclip" size={15} /> {t.attachments.attach}
    </button>
  );
}

/** Rust forgets these picked files; a failure only means they wait for the lock. */
function release(tokens: string[]) {
  if (tokens.length) void api.discardPendingFiles(tokens).catch(() => undefined);
}

/**
 * An item's files while it is edited: the saved ones it keeps, the ids
 * marked for removal and the files picked since, whose bytes Rust holds
 * until Save. A save takes the picked files whether or not it succeeds;
 * the editor discards those it still holds when it closes.
 */
export function useFileEdits(saved: AttachmentMeta[] = [], onError: (e: unknown) => void) {
  const { t } = useI18n();
  const [removed, setRemoved] = useState<string[]>([]);
  const [pending, setPending] = useState<PendingFile[]>([]);
  const [picking, setPicking] = useState(false);
  // The tokens Rust still holds for this editor.
  const held = useRef<string[]>([]);
  const open = useRef(true);

  useEffect(() => {
    open.current = true;
    return () => {
      open.current = false;
      release(held.current.splice(0));
    };
  }, []);

  const rows: FileRow[] = [
    ...saved.filter((a) => !removed.includes(a.id)).map((a) => ({ key: a.id, name: a.name, size: a.size, mime: a.mime, pending: false })),
    ...pending.map((p) => ({ key: p.token, name: p.name, size: p.size, mime: p.mime, pending: true })),
  ];

  async function attach() {
    setPicking(true);
    try {
      const picked = await api.pickFiles(rows.length);
      const tokens = picked.map((p) => p.token);
      if (!open.current) return release(tokens);
      held.current.push(...tokens);
      setPending((list) => [...list, ...picked]);
    } catch (e) {
      onError(e);
    } finally {
      setPicking(false);
    }
  }

  function remove(row: FileRow) {
    if (!row.pending) return setRemoved((ids) => [...ids, row.key]);
    held.current = held.current.filter((token) => token !== row.key);
    setPending((list) => list.filter((p) => p.token !== row.key));
    release([row.key]);
  }

  /** The edit to send with a save, or undefined when the files are unchanged. Rust takes the picked files. */
  function take(): FilesEdit | undefined {
    const added = held.current.splice(0);
    return added.length || removed.length ? { added, removed } : undefined;
  }

  /**
   * After a failed save: its picked files are gone, so their rows go and the
   * message says so once. A file Rust had already dropped (the vault locked)
   * needs no other words.
   */
  function saveFailed(err: unknown): string {
    setPending([]);
    if (pending.length === 0) return errorMessage(err, t, t.editor.saveFailed);
    const stale = err instanceof ApiError && err.code === "invalid_input" && err.message.includes("This file is no longer available");
    return stale ? t.attachments.notSaved : `${errorMessage(err, t, t.editor.saveFailed)} ${t.attachments.notSaved}`;
  }

  return {
    rows,
    picking,
    attach,
    remove,
    take,
    saveFailed,
    discard: () => release(held.current.splice(0)),
    dirty: removed.length > 0 || pending.length > 0,
  };
}

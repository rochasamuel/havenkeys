import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../lib/api";
import { formatSize } from "../lib/format";
import type { AttachmentMeta, ItemOverview } from "../lib/types";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { IconButton } from "./Field";
import { Icon } from "./Icon";
import { useToast } from "./Toast";

/** The types Rust sniffs as raster images, the only ones it lets us preview. */
const IMAGE_KINDS: Record<string, string> = {
  "image/png": "PNG",
  "image/jpeg": "JPEG",
  "image/webp": "WebP",
  "image/gif": "GIF",
};

/** Set once the user has read the save warning; lasts as long as the window. */
const SAVE_WARNED = "hk.attachments.saveWarned";

function saveWarned(): boolean {
  try {
    return sessionStorage.getItem(SAVE_WARNED) === "1";
  } catch {
    return false;
  }
}

function rememberSaveWarned() {
  try {
    sessionStorage.setItem(SAVE_WARNED, "1");
  } catch {
    // Unavailable storage only means the warning shows again.
  }
}

const isImage = (a: AttachmentMeta) => Object.hasOwn(IMAGE_KINDS, a.mime);

type Confirm = { kind: "remove" | "save"; id: string } | null;

/**
 * An item's attached files (spec 2026-10-13-encrypted-attachments §4.9).
 * Only names, types and sizes are here; bytes reach the webview only for an
 * image the user asks to preview. Picking, saving and reading files is done
 * by Rust behind its own dialogs.
 */
export function Attachments({ item, readOnly }: { item: ItemOverview; readOnly: boolean }) {
  const { t, dateLocale } = useI18n();
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<Confirm>(null);
  const preview = usePreview(item.id);
  const files = item.attachments ?? [];

  async function run(action: () => Promise<unknown>, fallback: string) {
    setBusy(true);
    try {
      await action();
    } catch (e) {
      toast(errorMessage(e, t, fallback), "error");
    } finally {
      setBusy(false);
    }
  }

  const add = () => run(() => api.addAttachments(item.id), t.attachments.addFailed);

  const save = (a: AttachmentMeta) => {
    if (!saveWarned()) return setConfirm({ kind: "save", id: a.id });
    setConfirm(null);
    return run(async () => {
      if (await api.saveAttachment(item.id, a.id)) toast(t.attachments.saved);
    }, t.attachments.saveFailed);
  };

  const remove = (a: AttachmentMeta) =>
    run(async () => {
      await api.removeAttachment(item.id, a.id);
      setConfirm(null);
    }, t.attachments.removeFailed);

  const kind = (a: AttachmentMeta) =>
    IMAGE_KINDS[a.mime] ?? (a.mime === "application/pdf" ? "PDF" : t.attachments.file);

  if (files.length === 0 && readOnly) return null;
  return (
    <section aria-label={t.attachments.title}>
      {files.length > 0 && <h3 className="group-title">{t.attachments.title}</h3>}
      <ul className="group attachment-list">
        {files.map((a) => (
          <li className="row attachment-row" key={a.id}>
            <Icon name={isImage(a) ? "image" : "file"} size={18} className="attachment-icon" />
            <div className="row-main">
              <div className="row-value attachment-name selectable" title={a.name} data-truncate="">
                {a.name}
              </div>
              <div className="attachment-meta">
                {kind(a)} · {formatSize(a.size, dateLocale)}
              </div>
            </div>
            <div className="row-actions-inline">
              {isImage(a) && (
                <IconButton
                  icon="eye"
                  label={t.attachments.preview(a.name)}
                  onClick={() => void preview.open(a).catch((e) => toast(errorMessage(e, t, t.attachments.previewFailed), "error"))}
                />
              )}
              <IconButton icon="download" label={t.attachments.save(a.name)} onClick={() => void save(a)} disabled={busy} />
              <IconButton
                icon="trash"
                label={t.attachments.remove(a.name)}
                onClick={() => setConfirm({ kind: "remove", id: a.id })}
                disabled={readOnly || busy}
              />
            </div>
            {confirm?.id === a.id && (
              <div className="confirm attachment-confirm" role="alert">
                <span>{confirm.kind === "remove" ? t.attachments.confirmRemove(a.name) : t.attachments.saveWarning}</span>
                <button className="btn btn-small" onClick={() => setConfirm(null)} autoFocus>
                  {t.common.cancel}
                </button>
                {confirm.kind === "remove" ? (
                  <button className="btn btn-small btn-danger" onClick={() => void remove(a)} disabled={busy || readOnly}>
                    {t.common.remove}
                  </button>
                ) : (
                  <button
                    className="btn btn-small btn-primary"
                    onClick={() => {
                      rememberSaveWarned();
                      void save(a);
                    }}
                    disabled={busy}
                  >
                    {t.attachments.saveAnyway}
                  </button>
                )}
              </div>
            )}
          </li>
        ))}
        {!readOnly && (
          <li>
            <button className="row row-button attachment-add" onClick={() => void add()} disabled={busy}>
              <Icon name="paperclip" size={16} />
              <span className="row-label-inline">{busy ? t.attachments.adding : t.attachments.add}</span>
            </button>
          </li>
        )}
      </ul>
      {preview.shown && <PreviewModal name={preview.shown.name} url={preview.shown.url} onClose={preview.close} />}
    </section>
  );
}

/**
 * The previewed image as a `blob:` URL. It is revoked when the preview
 * closes, when another item opens, on unmount and when the vault locks; a
 * fetch that lands after any of those is dropped unseen.
 */
function usePreview(itemId: string) {
  const [shown, setShown] = useState<{ name: string; url: string } | null>(null);
  const generation = useRef(0);

  const close = useCallback(() => {
    generation.current++;
    setShown(null);
  }, []);

  useEffect(() => {
    if (!shown) return;
    return () => URL.revokeObjectURL(shown.url);
  }, [shown]);

  useEffect(() => {
    const off = api.onLocked(close);
    return () => {
      generation.current++;
      void off.then((f) => f());
    };
  }, [close]);

  useEffect(() => close, [itemId, close]);

  const open = async (a: AttachmentMeta) => {
    const mine = ++generation.current;
    const bytes = await api.previewAttachment(itemId, a.id);
    if (mine !== generation.current) return;
    setShown({ name: a.name, url: URL.createObjectURL(new Blob([bytes], { type: a.mime })) });
  };

  return { shown, open, close };
}

/** The image, alone, over the app; Escape, the scrim or Close dismiss it. */
function PreviewModal({ name, url, onClose }: { name: string; url: string; onClose: () => void }) {
  const { t } = useI18n();
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const appRoot = document.getElementById("root");
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    appRoot?.setAttribute("inert", "");
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      onClose();
    };
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      appRoot?.removeAttribute("inert");
      if (previous?.isConnected) previous.focus();
    };
  }, [onClose]);

  return createPortal(
    <div className="modal-scrim" onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal preview-modal" role="dialog" aria-modal="true" aria-label={name}>
        <header className="preview-head">
          <h2 className="preview-name" data-truncate="">
            {name}
          </h2>
          <button ref={closeRef} type="button" className="icon-btn" aria-label={t.attachments.closePreview} title={t.attachments.closePreview} onClick={onClose}>
            <Icon name="x" size={16} />
          </button>
        </header>
        <img className="preview-image" src={url} alt={name} draggable={false} />
      </div>
    </div>,
    document.body,
  );
}

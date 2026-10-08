import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { TrashEntry } from "../lib/types";
import { cardSubtitle } from "../lib/card";
import { Field } from "../components/Field";
import { Icon } from "../components/Icon";
import { Seal } from "../components/Seal";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import type { Messages } from "../i18n/en";
import { ItemAvatar } from "./ItemList";

interface Props {
  /** Offline: restoring and deleting would fail, so those controls are disabled up front. */
  readOnly: boolean;
  /** Bumped when the vault reloads its items (a sync, the extension): the Trash reloads too. */
  revision: number;
  /** An item left the Trash: the vault reloads its items and the sidebar count. */
  onChanged: () => void;
}

type Confirm = { kind: "delete"; id: string } | { kind: "empty" } | null;

const DAY_MS = 86_400_000;

/** "Deleted 2 days ago · Removed in 28 days" */
function when(entry: TrashEntry, t: Messages): string {
  const ago = Math.max(0, Math.floor((Date.now() - entry.trashedAt) / DAY_MS));
  return `${t.trash.deletedAgo(ago)} · ${t.trash.removedIn(entry.daysLeft)}`;
}

function subtitle(entry: TrashEntry, t: Messages): string {
  if (entry.username) return entry.username;
  const card = entry.itemType === "card" ? cardSubtitle(entry.card) : null;
  if (card) return card;
  return entry.itemType === "card" ? t.card.kind : entry.itemType === "secure_note" ? t.common.secureNote : t.common.login;
}

/**
 * The Trash: overviews only. Nothing secret is fetched for a trashed item;
 * restoring it is the way back to its passwords and codes.
 */
export function TrashView({ readOnly, revision, onChanged }: Props) {
  const toast = useToast();
  const { t } = useI18n();
  const [entries, setEntries] = useState<TrashEntry[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<Confirm>(null);
  const [busy, setBusy] = useState(false);

  // Only the latest request may land: an older answer must not replace a newer one.
  const seq = useRef(0);
  const load = useCallback(async () => {
    const mine = ++seq.current;
    try {
      const list = await api.listTrash();
      if (mine !== seq.current) return;
      setEntries(list);
      setFailed(false);
    } catch {
      if (mine === seq.current) setFailed(true);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, revision]);

  const selected = entries?.find((e) => e.id === selectedId) ?? null;
  const count = entries?.length ?? 0;
  const anyPasskey = entries?.some((e) => e.hasPasskey) ?? false;

  async function runAction(run: () => Promise<void>, fallback: string) {
    setBusy(true);
    try {
      await run();
      setConfirm(null);
      onChanged();
    } catch (e) {
      toast(errorMessage(e, t, fallback), "error");
    } finally {
      setBusy(false);
      // Restored or deleted on another device meanwhile: show what is left.
      void load();
    }
  }

  const restore = (entry: TrashEntry) =>
    runAction(async () => {
      await api.restoreItem(entry.id);
      setSelectedId(null);
      toast(t.vault.restored(entry.title));
    }, t.trash.restoreFailed);

  const purge = (entry: TrashEntry) =>
    runAction(async () => {
      await api.purgeItem(entry.id);
      setSelectedId(null);
      toast(t.vault.deleted(entry.title));
    }, t.vault.deleteFailed);

  const empty = () =>
    runAction(async () => {
      await api.emptyTrash();
      setSelectedId(null);
    }, t.trash.emptyFailed);

  function confirmRow(text: string, passkeys: boolean, onConfirm: () => void) {
    return (
      <div className="confirm" role="alert">
        <span>
          {text}
          {passkeys && <> {t.trash.passkeyWarning}</>}
        </span>
        {/* Focus lands on the safe choice; the control that opened this is gone. */}
        <button className="btn btn-small" onClick={() => setConfirm(null)} disabled={busy} autoFocus>
          {t.common.cancel}
        </button>
        <button className="btn btn-small btn-danger" onClick={onConfirm} disabled={busy || readOnly}>
          {t.common.delete}
        </button>
      </div>
    );
  }

  return (
    <>
      <div className="list-col">
        <section className="list" aria-labelledby="trash-title">
          <header className="list-head" data-tauri-drag-region>
            <div className="list-title" data-tauri-drag-region>
              <h2 id="trash-title">{t.trash.title}</h2>
              {count > 0 && <span className="list-count">{count}</span>}
            </div>
            <button
              className="btn btn-small btn-quiet-danger"
              onClick={() => setConfirm({ kind: "empty" })}
              disabled={readOnly || busy || count === 0}
            >
              {t.trash.emptyTrash}
            </button>
          </header>
          <p className="trash-explain">{t.trash.explain}</p>
          {confirm?.kind === "empty" && (
            <div className="trash-confirm">{confirmRow(t.trash.confirmEmpty(count), anyPasskey, () => void empty())}</div>
          )}

          {entries === null ? (
            <p className="trash-status muted" role="status">
              {failed ? t.trash.loadFailed : t.common.loading}
            </p>
          ) : entries.length === 0 ? (
            <div className="list-empty">
              <Icon name="trash" size={22} />
              <p>{t.trash.empty}</p>
            </div>
          ) : (
            <ul className="list-items">
              {entries.map((entry) => (
                <li key={entry.id}>
                  <button
                    className="list-item"
                    aria-current={entry.id === selectedId ? "true" : undefined}
                    onClick={() => {
                      setSelectedId(entry.id);
                      if (confirm?.kind === "delete") setConfirm(null);
                    }}
                  >
                    <ItemAvatar item={entry} />
                    <span className="list-item-text">
                      {/* User data may be cut with an ellipsis (data-truncate); our own words never are. */}
                      <span className="list-item-title" data-truncate="">
                        {entry.title}
                      </span>
                      <span className="list-item-sub" data-truncate="">
                        {subtitle(entry, t)}
                      </span>
                      <span className="list-item-sub trash-when">{when(entry, t)}</span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      <section className="detail" aria-label={t.vault.details}>
        {selected ? (
          <article key={selected.id} className="item trash-item">
            <header className="item-head" data-tauri-drag-region>
              <ItemAvatar item={selected} large />
              <div className="item-head-text">
                <h2 className="item-title">{selected.title}</h2>
                <p className="item-kind">{when(selected, t)}</p>
              </div>
            </header>

            {(selected.hasPasskey || selected.tags.length > 0) && (
              <div className="detail-tags">
                {selected.hasPasskey && (
                  <span className="pill trash-pill">
                    <Icon name="key" size={12} />
                    {t.list.hasPasskeys}
                  </span>
                )}
                {selected.tags.map((name) => (
                  <span key={name} className="chip tag-chip">
                    <Icon name="tag" size={12} />
                    {name}
                  </span>
                ))}
              </div>
            )}

            {(selected.username || selected.urls.length > 0 || (selected.itemType === "card" && selected.card)) && (
              <div className="group">
                {selected.username && (
                  <Field label={t.common.username}>
                    <span className="selectable">{selected.username}</span>
                  </Field>
                )}
                {selected.itemType === "card" && cardSubtitle(selected.card) && (
                  <Field label={t.card.kind}>{cardSubtitle(selected.card)}</Field>
                )}
                {selected.urls.map((u) => (
                  <Field key={u.url} label={t.detail.website}>
                    <span className="selectable">{u.url}</span>
                  </Field>
                ))}
              </div>
            )}

            <p className="trash-restore-first">{t.trash.restoreFirst}</p>

            {confirm?.kind === "delete" && confirm.id === selected.id ? (
              confirmRow(t.trash.confirmDelete(selected.title), selected.hasPasskey, () => void purge(selected))
            ) : (
              <div className="trash-actions">
                <button className="btn btn-primary" onClick={() => void restore(selected)} disabled={readOnly || busy}>
                  {t.trash.restore}
                </button>
                <button
                  className="btn btn-quiet-danger"
                  onClick={() => setConfirm({ kind: "delete", id: selected.id })}
                  disabled={readOnly || busy}
                >
                  {t.trash.deleteForever}
                </button>
              </div>
            )}
          </article>
        ) : (
          // With nothing in the Trash the list already says so: the pane stays quiet.
          <div className="detail-empty">
            <Seal size={44} />
            {count > 0 && (
              <>
                <p className="detail-empty-title">{t.vault.nothingSelected}</p>
                <p className="muted">{t.vault.chooseItem}</p>
              </>
            )}
          </div>
        )}
      </section>
    </>
  );
}

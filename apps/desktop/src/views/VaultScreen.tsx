import { useCallback, useEffect, useMemo, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { ItemOverview, ItemType } from "../lib/types";
import { Icon, type IconName } from "../components/Icon";
import { Seal } from "../components/Seal";
import { useToast } from "../components/Toast";
import { ItemList } from "./ItemList";
import { ItemDetail } from "./ItemDetail";
import { ItemEditor } from "./ItemEditor";
import { GeneratorView } from "./GeneratorView";
import { SettingsView } from "./SettingsView";
import { useI18n } from "../i18n/context";
import type { Messages } from "../i18n/en";

export type Section = "all" | "login" | "secure_note" | "generator" | "settings";

type Pane =
  | { kind: "empty" }
  | { kind: "view"; id: string }
  | { kind: "edit"; id: string }
  | { kind: "new"; itemType: ItemType };

interface Props {
  damagedItems: number;
  unreadableItems: number;
  /** Offline: writes would fail, so the mutating controls are disabled up front. */
  readOnly: boolean;
  onLock: () => void;
}

const sections: Array<{ id: Section; label: (t: Messages) => string; icon: IconName }> = [
  { id: "all", label: (t) => t.vault.allItems, icon: "grid" },
  { id: "login", label: (t) => t.vault.logins, icon: "key" },
  { id: "secure_note", label: (t) => t.vault.secureNotes, icon: "note" },
];

export function VaultScreen({ damagedItems, unreadableItems, readOnly, onLock }: Props) {
  const toast = useToast();
  const { t } = useI18n();
  const [section, setSection] = useState<Section>("all");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<ItemOverview[]>([]);
  const [pane, setPane] = useState<Pane>({ kind: "empty" });
  const [unreadable, setUnreadable] = useState(unreadableItems);
  const [busyResync, setBusyResync] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setItems(await api.listItems(query));
    } catch (e) {
      if (e instanceof ApiError && e.code !== "locked") toast(e.message, "error");
    }
  }, [query, toast]);

  useEffect(() => {
    const timer = window.setTimeout(() => void refresh(), 120);
    return () => window.clearTimeout(timer);
  }, [refresh]);

  // Logins saved from the browser extension.
  useEffect(() => {
    const unlisten = api.onItemsChanged(() => void refresh());
    return () => void unlisten.then((f) => f());
  }, [refresh]);

  const refreshUnreadable = useCallback(async () => {
    try {
      const status = await api.status();
      setUnreadable(status.unreadableItems);
    } catch {
      // Best-effort: the banner just keeps its last known count.
    }
  }, []);

  // A pull can add, change or remove items behind the UI's back. The
  // unreadable count is re-read every time, not derived from the report:
  // a sync that only clears a previously-unreadable row reports 0 changes.
  useEffect(() => {
    const unlisten = api.onSynced((report) => {
      if (report.added + report.updated + report.deleted > 0) void refresh();
      void refreshUnreadable();
    });
    return () => void unlisten.then((f) => f());
  }, [refresh, refreshUnreadable]);

  // Once per unlock: `t` is left out of the dependencies so that changing
  // the language does not show it again.
  useEffect(() => {
    if (damagedItems > 0) toast(t.vault.damaged(damagedItems), "error");
  }, [damagedItems, toast]);

  async function redownload() {
    setBusyResync(true);
    try {
      await api.resync();
      await refreshUnreadable();
      await refresh();
    } catch (e) {
      toast(e instanceof ApiError ? e.message : t.vault.redownloadFailed, "error");
    } finally {
      setBusyResync(false);
    }
  }

  const visible = useMemo(
    () => (section === "login" || section === "secure_note" ? items.filter((i) => i.itemType === section) : items),
    [items, section],
  );
  const counts = useMemo(
    () => ({
      all: items.length,
      login: items.filter((i) => i.itemType === "login").length,
      secure_note: items.filter((i) => i.itemType === "secure_note").length,
    }),
    [items],
  );

  const selectedId = pane.kind === "view" || pane.kind === "edit" ? pane.id : null;
  const selected = items.find((i) => i.id === selectedId) ?? null;
  const isToolSection = section === "generator" || section === "settings";

  async function lock() {
    try {
      await api.lock();
    } finally {
      onLock();
    }
  }

  function onSaved(item: ItemOverview) {
    void refresh();
    setPane({ kind: "view", id: item.id });
    toast(t.vault.saved);
  }

  async function onDelete(item: ItemOverview) {
    try {
      await api.deleteItem(item.id);
      setPane({ kind: "empty" });
      await refresh();
      toast(t.vault.deleted(item.title));
    } catch (e) {
      toast(e instanceof ApiError ? e.message : t.vault.deleteFailed, "error");
    }
  }

  function newItem(itemType: ItemType) {
    if (isToolSection) setSection("all");
    setPane({ kind: "new", itemType });
  }

  const isMac = document.documentElement.dataset.platform === "mac";

  return (
    <div className="vault">
      <aside className="sidebar">
        <div className="sidebar-top" data-tauri-drag-region>
          <div className="brand" data-tauri-drag-region>
            <Seal size={22} />
            <span className="brand-name">HavenKeys</span>
          </div>
        </div>

        <label className="search">
          <Icon name="search" size={15} />
          <input
            type="search"
            placeholder={t.vault.search}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              if (isToolSection) setSection("all");
            }}
            spellCheck={false}
            autoComplete="off"
            aria-label={t.vault.searchLabel}
          />
        </label>

        <nav className="nav" aria-label={t.vault.sectionsLabel}>
          <p className="nav-heading">{t.vault.vaultHeading}</p>
          {sections.map((s) => (
            <button
              key={s.id}
              className="nav-item"
              aria-current={section === s.id ? "page" : undefined}
              onClick={() => setSection(s.id)}
            >
              <Icon name={s.icon} size={17} />
              <span>{s.label(t)}</span>
              <span className="nav-count">{counts[s.id as "all" | "login" | "secure_note"]}</span>
            </button>
          ))}
          <p className="nav-heading">{t.vault.toolsHeading}</p>
          <button
            className="nav-item"
            aria-current={section === "generator" ? "page" : undefined}
            onClick={() => setSection("generator")}
          >
            <Icon name="dice" size={17} />
            <span>{t.vault.generator}</span>
          </button>
          <button
            className="nav-item"
            aria-current={section === "settings" ? "page" : undefined}
            onClick={() => setSection("settings")}
          >
            <Icon name="gear" size={17} />
            <span>{t.vault.settings}</span>
          </button>
        </nav>

        <footer className="sidebar-foot">
          <div className={`conn${readOnly ? " is-offline" : ""}`} role="status">
            <Icon name={readOnly ? "cloudOff" : "cloud"} size={15} />
            <span>{readOnly ? t.vault.offline : t.vault.connected}</span>
          </div>
          <button className="lock-btn" onClick={() => void lock()} title={t.vault.lockTitle(isMac ? "⌘L" : "Ctrl+L")}>
            <span className="lock-btn-icon" aria-hidden="true">
              <Icon name="unlock" size={16} />
            </span>
            <span className="lock-btn-text">
              <strong>{t.vault.unlocked}</strong>
              <small>{t.vault.lockNow}</small>
            </span>
            <kbd>{isMac ? "⌘L" : "Ctrl L"}</kbd>
          </button>
        </footer>
      </aside>

      {section === "generator" && <GeneratorView />}
      {section === "settings" && <SettingsView onImported={() => void refresh()} online={!readOnly} />}

      {!isToolSection && (
        <>
          <div className="list-col">
            {unreadable > 0 && (
              <div className="banner banner-warn" role="status">
                {t.vault.unreadable(unreadable)}{" "}
                <button
                  className="btn btn-quiet"
                  type="button"
                  disabled={readOnly || busyResync}
                  onClick={() => void redownload()}
                >
                  {t.vault.redownload}
                </button>
              </div>
            )}
            <ItemList
              items={visible}
              query={query}
              section={section}
              selectedId={selectedId}
              onSelect={(id) => setPane({ kind: "view", id })}
              onNew={newItem}
              newDisabled={readOnly}
            />
          </div>
          <section className="detail" aria-label={t.vault.details}>
            {pane.kind === "empty" && (
              <div className="detail-empty">
                <Seal size={44} />
                <p className="detail-empty-title">{items.length === 0 ? t.vault.emptyTitle : t.vault.nothingSelected}</p>
                <p className="muted">
                  {items.length === 0 ? t.vault.emptyBody : t.vault.chooseItem}
                </p>
                {items.length === 0 && (
                  <div className="detail-empty-actions">
                    <button className="btn btn-primary" onClick={() => newItem("login")} disabled={readOnly}>
                      <Icon name="key" size={16} /> {t.vault.addLogin}
                    </button>
                    <button className="btn" onClick={() => newItem("secure_note")} disabled={readOnly}>
                      <Icon name="note" size={16} /> {t.vault.addNote}
                    </button>
                    <button className="btn" onClick={() => setSection("settings")} disabled={readOnly}>
                      {t.vault.import1Password}
                    </button>
                  </div>
                )}
              </div>
            )}
            {pane.kind === "view" && selected && (
              <ItemDetail
                key={selected.id + selected.updatedAt}
                item={selected}
                readOnly={readOnly}
                onEdit={() => setPane({ kind: "edit", id: selected.id })}
                onDelete={() => void onDelete(selected)}
              />
            )}
            {pane.kind === "edit" && selected && (
              <ItemEditor
                key={"edit" + selected.id}
                existing={selected}
                itemType={selected.itemType}
                readOnly={readOnly}
                onCancel={() => setPane({ kind: "view", id: selected.id })}
                onSaved={onSaved}
              />
            )}
            {pane.kind === "new" && (
              <ItemEditor
                key={"new" + pane.itemType}
                itemType={pane.itemType}
                readOnly={readOnly}
                onCancel={() => setPane({ kind: "empty" })}
                onSaved={onSaved}
              />
            )}
          </section>
        </>
      )}
    </div>
  );
}

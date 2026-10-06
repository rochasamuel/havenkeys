import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { AccountStatus, ItemOverview, ItemType } from "../lib/types";
import { showAccountItem } from "../lib/accountItem";
import { decideOpen } from "../lib/openItem";
import { Icon, type IconName } from "../components/Icon";
import { Seal } from "../components/Seal";
import { useToast } from "../components/Toast";
import { ItemList } from "./ItemList";
import { ItemDetail } from "./ItemDetail";
import { AccountItemDetail } from "./AccountItemDetail";
import { CardDetail } from "./CardDetail";
import { CardEditor } from "./CardEditor";
import { IdentityDetail } from "./IdentityDetail";
import { IdentityEditor } from "./IdentityEditor";
import { ItemEditor } from "./ItemEditor";
import { GeneratorView } from "./GeneratorView";
import { SettingsView } from "./SettingsView";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import type { Messages } from "../i18n/en";

export type Section = "all" | "login" | "secure_note" | "card" | "generator" | "settings";

type Pane =
  | { kind: "empty" }
  | { kind: "view"; id: string }
  | { kind: "edit"; id: string }
  | { kind: "new"; itemType: ItemType }
  | { kind: "account" };

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
  { id: "card", label: (t) => t.card.nav, icon: "card" },
];

export function VaultScreen({ damagedItems, unreadableItems, readOnly, onLock }: Props) {
  const toast = useToast();
  const { t } = useI18n();
  const [section, setSection] = useState<Section>("all");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<ItemOverview[]>([]);
  /** Bumped each time the items are reloaded; lets a detail row follow other items. */
  const [revision, setRevision] = useState(0);
  // Every item, whatever the search: an open item and the login a "Sign in
  // with" row links to must not vanish because they don't match the query.
  const [allItems, setAllItems] = useState<ItemOverview[]>([]);
  const [pane, setPane] = useState<Pane>({ kind: "empty" });
  const [unreadable, setUnreadable] = useState(unreadableItems);
  const [busyResync, setBusyResync] = useState(false);
  const [editorDirty, setEditorDirty] = useState(false);
  const [pendingOpen, setPendingOpen] = useState<string | null>(null);
  const [account, setAccount] = useState<AccountStatus | null>(null);
  const [identityId, setIdentityId] = useState<string | null>(null);

  // The account's one Identity: created on this or another device at
  // connect, so it may appear later through a sync.
  useEffect(() => {
    api.identityItemId().then(setIdentityId, () => setIdentityId(null));
  }, []);

  // The HavenKeys Account item is built from the account record; no secret
  // is fetched until the user reveals or copies the Secret Key.
  useEffect(() => {
    api.accountStatus().then(setAccount, () => setAccount(null));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const searching = query.trim() !== "";
      const [found, all] = await Promise.all([api.listItems(query), searching ? api.listItems() : null]);
      setItems(found);
      setRevision((r) => r + 1);
      setAllItems(all ?? found);
    } catch (e) {
      if (e instanceof ApiError && e.code !== "locked") toast(errorMessage(e, t), "error");
    }
  }, [query, toast, t]);

  useEffect(() => {
    const timer = window.setTimeout(() => void refresh(), 120);
    return () => window.clearTimeout(timer);
  }, [refresh]);

  // Logins saved from the browser extension.
  useEffect(() => {
    const unlisten = api.onItemsChanged(() => void refresh());
    return () => void unlisten.then((f) => f());
  }, [refresh]);

  const openForEdit = useCallback((id: string) => {
    setPendingOpen(null);
    setSection("login");
    setQuery("");
    setPane({ kind: "edit", id });
  }, []);

  // Read by the onOpenItem listener below, which subscribes once and must
  // not close over stale state (finding 3): kept current on every render
  // instead of being in that effect's dependencies.
  const paneRef = useRef(pane);
  paneRef.current = pane;
  const editorDirtyRef = useRef(editorDirty);
  editorDirtyRef.current = editorDirty;
  const sectionRef = useRef(section);
  sectionRef.current = section;

  // "Edit in HavenKeys" from the browser extension's popup. Subscribes once
  // (stable deps): re-subscribing on every pane/editorDirty change would
  // leave a window, between the async unlisten and the new listen, with a
  // stale-closure listener or briefly no listener at all.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void api.onOpenItem((id) => {
      const pane = paneRef.current;
      // The Account item has no editor to lose: it counts as an empty pane.
      const ref =
        pane.kind === "new" ? { kind: "new" as const } : pane.kind === "account" ? { kind: "empty" as const } : pane;
      const editorShown = sectionRef.current !== "generator" && sectionRef.current !== "settings";
      switch (decideOpen(ref, editorDirtyRef.current, id, editorShown)) {
        case "already":
          return;
        case "reveal":
          // The editor for this item is already open, just hidden behind
          // Settings/Generator: bring it back without touching its state.
          setSection("login");
          return;
        case "confirm":
          setPendingOpen(id);
          return;
        case "open":
          openForEdit(id);
      }
    }).then((f) => {
      if (cancelled) f();
      else unlisten = f;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [openForEdit]);

  // The discard-changes banner only makes sense while an edit/new pane is
  // showing; if the user saves or cancels instead of choosing the banner's
  // buttons, drop the stale pending request so it can't resurface later.
  useEffect(() => {
    if (pane.kind !== "edit" && pane.kind !== "new") setPendingOpen(null);
  }, [pane]);

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
      toast(errorMessage(e, t, t.vault.redownloadFailed), "error");
    } finally {
      setBusyResync(false);
    }
  }

  const visible = useMemo(
    () => (section === "login" || section === "secure_note" || section === "card" ? items.filter((i) => i.itemType === section) : items),
    [items, section],
  );
  const counts = useMemo(
    () => ({
      all: items.length,
      login: items.filter((i) => i.itemType === "login").length,
      secure_note: items.filter((i) => i.itemType === "secure_note").length,
      card: items.filter((i) => i.itemType === "card").length,
    }),
    [items],
  );

  const selectedId = pane.kind === "view" || pane.kind === "edit" ? pane.id : null;
  const selected = allItems.find((i) => i.id === selectedId) ?? null;
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
      toast(errorMessage(e, t, t.vault.deleteFailed), "error");
    }
  }

  const identity = identityId ? (allItems.find((i) => i.id === identityId) ?? null) : null;

  // The Identity entry, not All items, is the current place while it is open.
  const identitySelected = !isToolSection && identity !== null && selectedId === identity.id;

  function openIdentity() {
    if (!identity) return;
    setSection("all");
    setQuery("");
    setPane({ kind: "view", id: identity.id });
  }

  // "Show Emergency Kit" on the Account item: Settings, scrolled to the kit.
  function showKit() {
    setSection("settings");
    window.requestAnimationFrame(() =>
      document.getElementById("emergency-kit")?.scrollIntoView({ block: "start", behavior: "smooth" }),
    );
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
              aria-current={section === s.id && !identitySelected ? "page" : undefined}
              onClick={() => setSection(s.id)}
            >
              <Icon name={s.icon} size={17} />
              <span>{s.label(t)}</span>
              <span className="nav-count">{counts[s.id as "all" | "login" | "secure_note" | "card"]}</span>
            </button>
          ))}
          <button
            className="nav-item"
            aria-current={identitySelected ? "page" : undefined}
            onClick={openIdentity}
            disabled={!identity}
            title={identity ? undefined : t.identity.notCreated}
          >
            <Icon name="idCard" size={17} />
            <span>{t.identity.nav}</span>
          </button>
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
              account={showAccountItem(account, section, query, t.accountItem.title) ? account : null}
              accountSelected={pane.kind === "account"}
              onSelectAccount={() => setPane({ kind: "account" })}
            />
          </div>
          <section className="detail" aria-label={t.vault.details}>
            {pendingOpen && (pane.kind === "edit" || pane.kind === "new") && (
              <div className="confirm open-confirm" role="alert">
                <span>
                  {pane.kind === "edit" && selected ? t.vault.discardChanges(selected.title) : t.vault.discardNewItem}
                </span>
                <button className="btn btn-small" onClick={() => setPendingOpen(null)}>
                  {t.vault.keepEditing}
                </button>
                <button className="btn btn-small btn-danger" onClick={() => openForEdit(pendingOpen)}>
                  {t.vault.discard}
                </button>
              </div>
            )}
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
                      {t.vault.importOther}
                    </button>
                  </div>
                )}
              </div>
            )}
            {pane.kind === "account" && account && (
              <AccountItemDetail account={account} onShowKit={showKit} />
            )}
            {pane.kind === "view" && selected?.itemType === "identity" && (
              <IdentityDetail
                key={selected.id + selected.updatedAt}
                item={selected}
                readOnly={readOnly}
                onEdit={() => setPane({ kind: "edit", id: selected.id })}
              />
            )}
            {pane.kind === "edit" && selected?.itemType === "identity" && (
              <IdentityEditor
                key={"edit" + selected.id}
                existing={selected}
                readOnly={readOnly}
                onCancel={() => setPane({ kind: "view", id: selected.id })}
                onSaved={onSaved}
                onDirtyChange={setEditorDirty}
              />
            )}
            {pane.kind === "view" && selected?.itemType === "card" && (
              <CardDetail
                key={selected.id + selected.updatedAt}
                item={selected}
                readOnly={readOnly}
                onEdit={() => setPane({ kind: "edit", id: selected.id })}
                onDelete={() => void onDelete(selected)}
              />
            )}
            {pane.kind === "edit" && selected?.itemType === "card" && (
              <CardEditor
                key={"edit" + selected.id}
                existing={selected}
                readOnly={readOnly}
                onCancel={() => setPane({ kind: "view", id: selected.id })}
                onSaved={onSaved}
                onDirtyChange={setEditorDirty}
              />
            )}
            {pane.kind === "view" && selected && selected.itemType !== "identity" && selected.itemType !== "card" && (
              <ItemDetail
                key={selected.id + selected.updatedAt}
                item={selected}
                revision={revision}
                readOnly={readOnly}
                onEdit={() => setPane({ kind: "edit", id: selected.id })}
                onDelete={() => void onDelete(selected)}
                onOpen={(id) => setPane({ kind: "view", id })}
              />
            )}
            {pane.kind === "edit" && selected && selected.itemType !== "identity" && selected.itemType !== "card" && (
              <ItemEditor
                key={"edit" + selected.id}
                existing={selected}
                itemType={selected.itemType}
                readOnly={readOnly}
                onCancel={() => setPane({ kind: "view", id: selected.id })}
                onSaved={onSaved}
                onDirtyChange={setEditorDirty}
              />
            )}
            {pane.kind === "new" && pane.itemType !== "card" && (
              <ItemEditor
                key={"new" + pane.itemType}
                itemType={pane.itemType}
                readOnly={readOnly}
                onCancel={() => setPane({ kind: "empty" })}
                onSaved={onSaved}
                onDirtyChange={setEditorDirty}
              />
            )}
            {pane.kind === "new" && pane.itemType === "card" && (
              <CardEditor
                key="new-card"
                readOnly={readOnly}
                onCancel={() => setPane({ kind: "empty" })}
                onSaved={onSaved}
                onDirtyChange={setEditorDirty}
              />
            )}
          </section>
        </>
      )}
    </div>
  );
}

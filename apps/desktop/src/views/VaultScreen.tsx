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

export type Section = "all" | "login" | "secure_note" | "generator" | "settings";

type Pane =
  | { kind: "empty" }
  | { kind: "view"; id: string }
  | { kind: "edit"; id: string }
  | { kind: "new"; itemType: ItemType };

interface Props {
  damagedItems: number;
  /** Offline: writes would fail, so the mutating controls are disabled up front. */
  readOnly: boolean;
  onLock: () => void;
}

const sections: Array<{ id: Section; label: string; icon: IconName }> = [
  { id: "all", label: "All items", icon: "grid" },
  { id: "login", label: "Logins", icon: "key" },
  { id: "secure_note", label: "Secure notes", icon: "note" },
];

export function VaultScreen({ damagedItems, readOnly, onLock }: Props) {
  const toast = useToast();
  const [section, setSection] = useState<Section>("all");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<ItemOverview[]>([]);
  const [pane, setPane] = useState<Pane>({ kind: "empty" });

  const refresh = useCallback(async () => {
    try {
      setItems(await api.listItems(query));
    } catch (e) {
      if (e instanceof ApiError && e.code !== "locked") toast(e.message, "error");
    }
  }, [query, toast]);

  useEffect(() => {
    const t = window.setTimeout(() => void refresh(), 120);
    return () => window.clearTimeout(t);
  }, [refresh]);

  // Logins saved from the browser extension.
  useEffect(() => {
    const unlisten = api.onItemsChanged(() => void refresh());
    return () => void unlisten.then((f) => f());
  }, [refresh]);

  // A pull can add, change or remove items behind the UI's back.
  useEffect(() => {
    const unlisten = api.onSynced((report) => {
      if (report.added + report.updated + report.deleted > 0) void refresh();
    });
    return () => void unlisten.then((f) => f());
  }, [refresh]);

  useEffect(() => {
    if (damagedItems > 0) {
      toast(`${damagedItems} item(s) could not be decrypted and are hidden.`, "error");
    }
  }, [damagedItems, toast]);

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
    toast("Saved.");
  }

  async function onDelete(item: ItemOverview) {
    try {
      await api.deleteItem(item.id);
      setPane({ kind: "empty" });
      await refresh();
      toast(`Deleted “${item.title}”.`);
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not delete the item.", "error");
    }
  }

  function newItem(itemType: ItemType) {
    if (isToolSection) setSection("all");
    setPane({ kind: "new", itemType });
  }

  return (
    <div className="vault">
      <aside className="sidebar">
        <div className="brand">
          <Seal size={28} />
          <span className="brand-name">HavenKeys</span>
        </div>

        <button className="latch" onClick={() => void lock()} title="Lock now (Ctrl+L)">
          <span className="latch-track" aria-hidden="true">
            <span className="latch-knob" />
          </span>
          <span className="latch-text">
            <strong>Unlocked</strong>
            <small>Lock now</small>
          </span>
        </button>

        <label className="search">
          <Icon name="search" size={16} />
          <input
            type="search"
            placeholder="Search vault"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              if (isToolSection) setSection("all");
            }}
            spellCheck={false}
            autoComplete="off"
            aria-label="Search vault"
          />
        </label>

        <nav className="nav" aria-label="Vault sections">
          {sections.map((s) => (
            <button
              key={s.id}
              className="nav-item"
              aria-current={section === s.id ? "page" : undefined}
              onClick={() => setSection(s.id)}
            >
              <Icon name={s.icon} size={17} />
              <span>{s.label}</span>
              <span className="nav-count">{counts[s.id as "all" | "login" | "secure_note"]}</span>
            </button>
          ))}
          <div className="nav-gap" />
          <button
            className="nav-item"
            aria-current={section === "generator" ? "page" : undefined}
            onClick={() => setSection("generator")}
          >
            <Icon name="dice" size={17} />
            <span>Password generator</span>
          </button>
          <button
            className="nav-item"
            aria-current={section === "settings" ? "page" : undefined}
            onClick={() => setSection("settings")}
          >
            <Icon name="gear" size={17} />
            <span>Settings</span>
          </button>
        </nav>
      </aside>

      {section === "generator" && <GeneratorView />}
      {section === "settings" && <SettingsView onImported={() => void refresh()} online={!readOnly} />}

      {!isToolSection && (
        <>
          <ItemList
            items={visible}
            query={query}
            section={section}
            selectedId={selectedId}
            onSelect={(id) => setPane({ kind: "view", id })}
            onNew={newItem}
            newDisabled={readOnly}
          />
          <section className="detail" aria-label="Item details">
            {pane.kind === "empty" && (
              <div className="detail-empty">
                <p>{items.length === 0 ? "Your vault is empty." : "Select an item to see its details."}</p>
                {items.length === 0 && (
                  <div className="detail-empty-actions">
                    <button className="btn btn-primary" onClick={() => newItem("login")} disabled={readOnly}>
                      <Icon name="key" size={16} /> Add a login
                    </button>
                    <button className="btn" onClick={() => newItem("secure_note")} disabled={readOnly}>
                      <Icon name="note" size={16} /> Add a secure note
                    </button>
                    <button className="btn" onClick={() => setSection("settings")} disabled={readOnly}>
                      Import from 1Password
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

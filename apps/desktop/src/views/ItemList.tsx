import { useEffect, useRef, useState } from "react";
import type { ItemOverview, ItemType } from "../lib/types";
import { monogram, primaryHost } from "../lib/format";
import { Icon } from "../components/Icon";
import type { Section } from "./VaultScreen";

interface Props {
  items: ItemOverview[];
  query: string;
  section: Section;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onNew: (type: ItemType) => void;
  /** Offline: creating an item would fail, so the control is disabled up front. */
  newDisabled?: boolean;
}

const headings: Partial<Record<Section, string>> = {
  all: "All items",
  login: "Logins",
  secure_note: "Secure notes",
};

export function ItemList({ items, query, section, selectedId, onSelect, onNew, newDisabled }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close the New menu on Escape or a click anywhere else.
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

  return (
    <section className="list" aria-label="Items">
      <header className="list-head" data-tauri-drag-region>
        <div className="list-title" data-tauri-drag-region>
          <h2>{query.trim() ? "Results" : headings[section]}</h2>
          <span className="list-count">{items.length}</span>
        </div>
        <div className="new-menu" ref={menuRef}>
          <button
            className="icon-btn icon-btn-solid"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            aria-label="New item"
            title="New item"
            onClick={() => setMenuOpen((o) => !o)}
            disabled={newDisabled}
          >
            <Icon name="plus" size={17} />
          </button>
          {menuOpen && !newDisabled && (
            <div className="menu" role="menu">
              <p className="menu-heading">New item</p>
              <button
                role="menuitem"
                onClick={() => {
                  setMenuOpen(false);
                  onNew("login");
                }}
              >
                <Icon name="key" size={16} /> Login
              </button>
              <button
                role="menuitem"
                onClick={() => {
                  setMenuOpen(false);
                  onNew("secure_note");
                }}
              >
                <Icon name="note" size={16} /> Secure note
              </button>
            </div>
          )}
        </div>
      </header>

      {items.length === 0 ? (
        <div className="list-empty">
          <Icon name={query.trim() ? "search" : "grid"} size={22} />
          <p>{query.trim() ? "No matches" : "Nothing here yet"}</p>
          <span>
            {query.trim() ? "Search looks at titles, usernames and websites." : "New items you add appear here."}
          </span>
        </div>
      ) : (
        <ul className="list-items">
          {items.map((item) => {
            const sub =
              item.itemType === "login" ? (item.username ?? primaryHost(item) ?? "Login") : "Secure note";
            return (
              <li key={item.id}>
                <button
                  className="list-item"
                  aria-current={item.id === selectedId ? "true" : undefined}
                  onClick={() => onSelect(item.id)}
                >
                  <span className={`avatar avatar-${item.itemType}`} aria-hidden="true">
                    {item.itemType === "secure_note" ? <Icon name="note" size={15} /> : monogram(item.title)}
                  </span>
                  <span className="list-item-text">
                    <span className="list-item-title">{item.title}</span>
                    <span className="list-item-sub">{sub}</span>
                  </span>
                  {item.hasTotp && (
                    <span className="list-item-flag" title="Has one-time codes">
                      <Icon name="clock" size={13} />
                    </span>
                  )}
                  {item.hasPasskey && (
                    <span className="list-item-flag" title="Has passkeys">
                      <Icon name="key" size={13} />
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

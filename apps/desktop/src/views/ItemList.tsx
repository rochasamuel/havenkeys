import { useEffect, useRef, useState } from "react";
import type { ItemOverview, ItemType } from "../lib/types";
import { monogram, primaryHost } from "../lib/format";
import { Icon } from "../components/Icon";
import type { Section } from "./VaultScreen";
import { useI18n } from "../i18n/context";

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

export function ItemList({ items, query, section, selectedId, onSelect, onNew, newDisabled }: Props) {
  const { t } = useI18n();
  const [menuOpen, setMenuOpen] = useState(false);
  const headings: Partial<Record<Section, string>> = {
    all: t.vault.allItems,
    login: t.vault.logins,
    secure_note: t.vault.secureNotes,
  };
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
    <section className="list" aria-label={t.list.items}>
      <header className="list-head" data-tauri-drag-region>
        <div className="list-title" data-tauri-drag-region>
          <h2>{query.trim() ? t.list.results : headings[section]}</h2>
          <span className="list-count">{items.length}</span>
        </div>
        <div className="new-menu" ref={menuRef}>
          <button
            className="icon-btn icon-btn-solid"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            aria-label={t.list.newItem}
            title={t.list.newItem}
            onClick={() => setMenuOpen((o) => !o)}
            disabled={newDisabled}
          >
            <Icon name="plus" size={17} />
          </button>
          {menuOpen && !newDisabled && (
            <div className="menu" role="menu">
              <p className="menu-heading">{t.list.newItem}</p>
              <button
                role="menuitem"
                onClick={() => {
                  setMenuOpen(false);
                  onNew("login");
                }}
              >
                <Icon name="key" size={16} /> {t.common.login}
              </button>
              <button
                role="menuitem"
                onClick={() => {
                  setMenuOpen(false);
                  onNew("secure_note");
                }}
              >
                <Icon name="note" size={16} /> {t.common.secureNote}
              </button>
            </div>
          )}
        </div>
      </header>

      {items.length === 0 ? (
        <div className="list-empty">
          <Icon name={query.trim() ? "search" : "grid"} size={22} />
          <p>{query.trim() ? t.list.noMatches : t.list.nothingYet}</p>
          <span>
            {query.trim() ? t.list.searchHint : t.list.emptyHint}
          </span>
        </div>
      ) : (
        <ul className="list-items">
          {items.map((item) => {
            const data = item.itemType === "login" ? (item.username ?? primaryHost(item)) : null;
            const sub = data ?? (item.itemType === "login" ? t.common.login : t.common.secureNote);
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
                    {/* User data may be cut with an ellipsis (data-truncate); our own words never are. */}
                    <span className="list-item-title" data-truncate="">
                      {item.title}
                    </span>
                    <span className="list-item-sub" data-truncate={data !== null ? "" : undefined}>
                      {sub}
                    </span>
                  </span>
                  {item.hasTotp && (
                    <span className="list-item-flag" title={t.list.hasTotp}>
                      <Icon name="clock" size={13} />
                    </span>
                  )}
                  {item.hasPasskey && (
                    <span className="list-item-flag" title={t.list.hasPasskeys}>
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

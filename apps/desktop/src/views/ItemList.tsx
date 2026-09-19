import { useState } from "react";
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
}

const headings: Partial<Record<Section, string>> = {
  all: "All items",
  login: "Logins",
  secure_note: "Secure notes",
};

export function ItemList({ items, query, section, selectedId, onSelect, onNew }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <section className="list" aria-label="Items">
      <header className="list-head">
        <h2>{query.trim() ? "Search results" : headings[section]}</h2>
        <div className="new-menu">
          <button
            className="btn btn-primary btn-small"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((o) => !o)}
          >
            <Icon name="plus" size={16} /> New
          </button>
          {menuOpen && (
            <div className="menu" role="menu" onMouseLeave={() => setMenuOpen(false)}>
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
        <p className="list-empty">
          {query.trim() ? "No items match your search. Search looks at titles, usernames and websites." : "Nothing here yet."}
        </p>
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
                    {item.itemType === "secure_note" ? <Icon name="note" size={16} /> : monogram(item.title)}
                  </span>
                  <span className="list-item-text">
                    <span className="list-item-title">{item.title}</span>
                    <span className="list-item-sub">{sub}</span>
                  </span>
                  {item.hasTotp && (
                    <span className="list-item-flag" title="Has one-time codes">
                      <Icon name="clock" size={14} />
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

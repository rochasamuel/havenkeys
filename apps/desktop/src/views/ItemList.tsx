import { useEffect, useRef, useState } from "react";
import type { AccountStatus, ItemOverview, ItemType } from "../lib/types";
import { monogram, primaryHost } from "../lib/format";
import { ssoSubtitle } from "../lib/sso";
import { identityTitle } from "../lib/identity";
import { Icon } from "../components/Icon";
import { Seal } from "../components/Seal";
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
  /** The HavenKeys Account row, pinned first, or null when it does not belong here. */
  account: AccountStatus | null;
  accountSelected: boolean;
  onSelectAccount: () => void;
}

export function ItemList({
  items,
  query,
  section,
  selectedId,
  onSelect,
  onNew,
  newDisabled,
  account,
  accountSelected,
  onSelectAccount,
}: Props) {
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

      {account && (
        <ul className="list-pinned">
          <li>
            <button
              className="list-item"
              aria-current={accountSelected ? "true" : undefined}
              onClick={onSelectAccount}
            >
              <span className="avatar avatar-account" aria-hidden="true">
                <Seal size={24} />
              </span>
              <span className="list-item-text">
                <span className="list-item-title">{t.accountItem.title}</span>
                <span className="list-item-sub" data-truncate="">
                  {account.email}
                </span>
              </span>
            </button>
          </li>
        </ul>
      )}

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
            const data =
              item.itemType === "login"
                ? (item.username ?? (item.signInWith ? ssoSubtitle(item.signInWith) : null) ?? primaryHost(item))
                : item.itemType === "identity"
                  ? item.username
                  : null;
            const sub =
              data ?? (item.itemType === "login" ? t.common.login : item.itemType === "identity" ? t.identity.kind : t.common.secureNote);
            const title = item.itemType === "identity" ? identityTitle(item.title, t.identity.title) : item.title;
            return (
              <li key={item.id}>
                <button
                  className="list-item"
                  aria-current={item.id === selectedId ? "true" : undefined}
                  onClick={() => onSelect(item.id)}
                >
                  <span className={`avatar avatar-${item.itemType}`} aria-hidden="true">
                    {item.itemType === "secure_note" ? (
                      <Icon name="note" size={15} />
                    ) : item.itemType === "identity" ? (
                      <Icon name="idCard" size={16} />
                    ) : (
                      monogram(item.title)
                    )}
                  </span>
                  <span className="list-item-text">
                    {/* User data may be cut with an ellipsis (data-truncate); our own words never are. */}
                    <span className="list-item-title" data-truncate="">
                      {title}
                    </span>
                    <span className="list-item-sub" data-truncate={data !== null ? "" : undefined}>
                      {sub}
                    </span>
                  </span>
                  {(item.hasTotp || item.hasPasskey) && (
                    // One element, so both flags share the grid's last column.
                    <span className="list-item-flags">
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

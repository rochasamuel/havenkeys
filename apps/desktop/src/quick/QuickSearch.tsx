import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { api } from "../lib/api";
import type { ItemOverview } from "../lib/types";
import { primaryHost } from "../lib/format";
import { ItemAvatar } from "../views/ItemList";
import { Icon } from "../components/Icon";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { quickKeyAction, type QuickKeyAction } from "./keys";

type Action = Exclude<QuickKeyAction, "hide" | "up" | "down">;

/**
 * Search and act. Holds overviews only (title, username, websites): every
 * action asks Rust, which copies the value itself (copy_secret) or opens a
 * saved website (open_website); the answer carries no secret.
 */
export function QuickSearch({ onHide }: { onHide: () => void }) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<ItemOverview[] | null>(null);
  const [index, setIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  // Only the answer for the latest query may land.
  const request = useRef(0);
  const mac = document.documentElement.dataset.platform === "mac";

  useEffect(() => inputRef.current?.focus(), []);

  useEffect(() => {
    const id = ++request.current;
    api.listItems(query).then(
      (all) => {
        if (id !== request.current) return;
        setItems(all.filter((i) => i.itemType === "login"));
        setIndex(0);
        setError(null);
      },
      (err) => {
        if (id === request.current) setError(errorMessage(err, t));
      },
    );
    // `t` is left out: a language change must not re-run the search.
  }, [query]);

  const current = items?.[index] ?? null;

  useEffect(() => {
    if (current) document.getElementById(`quick-row-${current.id}`)?.scrollIntoView?.({ block: "nearest" });
  }, [current]);

  function available(item: ItemOverview, action: Action): boolean {
    switch (action) {
      case "password":
        return item.hasPassword;
      case "username":
        return !!item.username;
      case "totp":
        return item.hasTotp;
      case "open":
        return item.urls.length > 0;
    }
  }

  async function run(item: ItemOverview, action: Action) {
    if (!available(item, action)) return;
    try {
      if (action === "open") {
        const url = item.urls[0]?.url;
        if (!url) return;
        await api.openWebsite(item.id, url);
      } else {
        // Rust copies the value; the answer is only the clear-after delay.
        await api.copy(item.id, action);
      }
      onHide();
    } catch (err) {
      setError(errorMessage(err, t, t.quick.actionFailed));
    }
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    // The field is the window's only focus: Tab would leave every key dead.
    if (e.key === "Tab") {
      e.preventDefault();
      return;
    }
    const action = quickKeyAction(e.nativeEvent, mac);
    if (action === null) return;
    if (action === "hide") {
      e.preventDefault();
      onHide();
      return;
    }
    if (action === "up" || action === "down") {
      e.preventDefault();
      const last = Math.max(0, (items?.length ?? 0) - 1);
      setIndex((i) => (action === "up" ? Math.max(0, i - 1) : Math.min(last, i + 1)));
      return;
    }
    // Nothing selected: Cmd/Ctrl+C copies selected text as usual.
    if (!current) return;
    e.preventDefault();
    void run(current, action);
  }

  const mod = mac ? "⌘" : "Ctrl+";
  return (
    <main
      className="quick"
      // A click on a row, the hints or the empty space keeps focus in the field.
      onMouseDown={(e) => {
        if (e.target !== inputRef.current) e.preventDefault();
      }}
    >
      <label className="quick-search">
        <Icon name="search" size={17} />
        <span className="sr-only">{t.quick.searchLabel}</span>
        <input
          ref={inputRef}
          role="combobox"
          aria-expanded={!!items?.length}
          aria-controls="quick-results"
          aria-activedescendant={current ? `quick-row-${current.id}` : undefined}
          aria-autocomplete="list"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={t.quick.searchPlaceholder}
          autoComplete="off"
          spellCheck={false}
          autoCapitalize="off"
        />
      </label>

      {items && items.length === 0 ? (
        <p className="quick-empty">{query.trim() ? t.quick.noResults : t.quick.noLogins}</p>
      ) : (
        <ul className="quick-list" id="quick-results" role="listbox" aria-label={t.quick.resultsLabel}>
          {items?.map((item, i) => (
            <li
              key={item.id}
              id={`quick-row-${item.id}`}
              role="option"
              aria-selected={i === index}
              className="list-item"
              onMouseMove={() => setIndex(i)}
              onClick={() => void run(item, "password")}
            >
              <ItemAvatar item={item} />
              <span className="list-item-text">
                <span className="list-item-title" data-truncate="">
                  {item.title}
                </span>
                <span className="list-item-sub" data-truncate="">
                  {item.username ?? primaryHost(item) ?? t.common.login}
                </span>
              </span>
            </li>
          ))}
        </ul>
      )}

      <p className={`quick-error${error ? " is-visible" : ""}`} role="alert">
        {error}
      </p>

      {/* Only the keys the selected login answers to. */}
      <footer className="quick-hints" aria-hidden="true">
        {current?.hasPassword && (
          <span>
            <kbd>↵</kbd> {t.quick.hints.password}
          </span>
        )}
        {current?.username && (
          <span>
            <kbd>{mod}C</kbd> {t.quick.hints.username}
          </span>
        )}
        {current?.hasTotp && (
          <span>
            <kbd>{mod}T</kbd> {t.quick.hints.totp}
          </span>
        )}
        {current && current.urls.length > 0 && (
          <span>
            <kbd>{mod}O</kbd> {t.quick.hints.open}
          </span>
        )}
        <span className="quick-hints-end">
          <kbd>esc</kbd> {t.quick.hints.close}
        </span>
      </footer>
    </main>
  );
}

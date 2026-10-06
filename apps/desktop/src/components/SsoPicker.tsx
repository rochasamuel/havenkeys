// "Sign in with" picker: each provider and, under it, the vault's own logins
// for that provider (from Rust's `sso_accounts`, no secrets). Picking a login
// copies its username as the account; the editor's account field stays
// editable for an account the vault does not hold.

import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import { applyRow, pickerRows, PROVIDER_NAMES, PROVIDER_ORDER, type PickerRow } from "../lib/sso";
import type { SignInWith, SsoAccount, SsoProvider } from "../lib/types";
import { useI18n } from "../i18n/context";
import { Icon } from "./Icon";
import { ProviderIcon } from "./ProviderIcon";

type Accounts = Partial<Record<SsoProvider, SsoAccount[]>>;

export function SsoPicker({ value, onChange }: { value: SignInWith | null; onChange: (v: SignInWith | null) => void }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [accounts, setAccounts] = useState<Accounts | null>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const loading = useRef(false);
  const loaded = useRef(new Set<SsoProvider>());
  const rows = useMemo(() => pickerRows(accounts ?? {}, query), [accounts, query]);

  // Loaded on open. Providers whose call succeeded are kept for the editor's
  // lifetime; failed ones (locked, offline) are retried on the next open.
  // One batch is in flight at a time, so reopening before it resolves does
  // not start another.
  useEffect(() => {
    if (!open || loading.current) return;
    const missing = PROVIDER_ORDER.filter((p) => !loaded.current.has(p));
    if (missing.length === 0) return;
    loading.current = true;
    void Promise.allSettled(missing.map((p) => api.ssoAccounts(p))).then((results) => {
      const out: Accounts = {};
      results.forEach((r, i) => {
        if (r.status !== "fulfilled") return;
        const p = missing[i] as SsoProvider;
        out[p] = r.value;
        loaded.current.add(p);
      });
      loading.current = false;
      setAccounts((cur) => ({ ...(cur ?? {}), ...out }));
    });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    document.getElementById(`sso-option-${active}`)?.scrollIntoView?.({ block: "nearest" });
  }, [open, active]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (wrap.current && !wrap.current.contains(e.target as Node)) close();
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  function close() {
    setOpen(false);
    setQuery("");
    setActive(0);
  }

  function pick(row: PickerRow) {
    onChange(applyRow(row, value));
    close();
    triggerRef.current?.focus();
  }

  function onKey(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.max(0, Math.min(a + 1, rows.length - 1)));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const row = rows[active];
      if (row) pick(row);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
      triggerRef.current?.focus();
    } else if (e.key === "Tab") {
      close();
    }
  }

  const rowId = (i: number) => `sso-option-${i}`;
  return (
    <div
      className="sso-picker"
      ref={wrap}
      onBlur={(e) => {
        if (open && !e.currentTarget.contains(e.relatedTarget as Node | null)) close();
      }}
    >
      <button
        ref={triggerRef}
        type="button"
        className="sso-trigger edit-input"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={t.editor.providerPick}
        onClick={() => (open ? close() : setOpen(true))}
      >
        {value ? (
          <>
            <ProviderIcon provider={value.provider} />
            <span data-truncate="">{PROVIDER_NAMES[value.provider]}</span>
            {value.account && <span className="muted" data-truncate="">· {value.account}</span>}
          </>
        ) : (
          <span className="muted" data-truncate="">{t.editor.providerNone}</span>
        )}
        <Icon name="chevronDown" size={14} className="select-chevron" />
      </button>
      {open && (
        <div className="menu sso-menu">
          <input
            type="search"
            role="combobox"
            aria-expanded="true"
            aria-autocomplete="list"
            className="edit-input sso-search"
            value={query}
            placeholder={t.editor.providerSearch}
            aria-label={t.editor.providerSearch}
            aria-controls="sso-listbox"
            aria-activedescendant={rows[active] ? rowId(active) : undefined}
            autoFocus
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            onKeyDown={onKey}
          />
          <div role="listbox" id="sso-listbox" aria-label={t.editor.providerPick}>
            {rows.map((row, i) => (
              <div
                key={row.kind === "account" ? `a-${row.account.id}` : row.kind === "provider" ? `p-${row.provider}` : "none"}
                id={rowId(i)}
                role="option"
                aria-selected={i === active}
                className={`sso-option${row.kind === "account" ? " sso-option-account" : ""}`}
                onMouseEnter={() => setActive(i)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => pick(row)}
              >
                {row.kind === "none" ? (
                  <span>{t.editor.providerNone}</span>
                ) : row.kind === "provider" ? (
                  <>
                    <ProviderIcon provider={row.provider} />
                    <span>{PROVIDER_NAMES[row.provider]}</span>
                  </>
                ) : (
                  <>
                    <ProviderIcon provider={row.provider} size={15} />
                    <span data-truncate="">{row.account.title}</span>
                    <span className="muted" data-truncate="">{row.account.username}</span>
                  </>
                )}
              </div>
            ))}
            {rows.length === 0 && <p className="sso-empty muted">{t.editor.noMatches}</p>}
          </div>
        </div>
      )}
    </div>
  );
}

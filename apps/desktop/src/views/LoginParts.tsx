// The parts of a login that a bank account shares: its websites (detail and
// editor) and the password row with the generator.

import { useState } from "react";
import { api } from "../lib/api";
import { KEEP, type SecretEdit } from "../lib/secretEdit";
import type { ItemOverview, MatchType, UrlRule } from "../lib/types";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

const matchTypes: MatchType[] = ["domain", "origin", "exact"];

/** The item's websites, each opened by Rust from the saved rule. */
export function Websites({ item }: { item: ItemOverview }) {
  const { t } = useI18n();
  const toast = useToast();
  if (item.urls.length === 0) return null;
  return (
    <div className="group">
      {item.urls.map((u) => (
        <div className="row" key={u.url}>
          <div className="row-main">
            <div className="row-label">{t.detail.website}</div>
            <div className="row-value url-value">
              <button
                type="button"
                className="url-link"
                title={t.detail.openWebsite}
                onClick={() =>
                  void api
                    .openWebsite(item.id, u.url)
                    .catch((e) => toast(errorMessage(e, t, t.detail.openWebsiteFailed), "error"))
                }
              >
                {u.url}
              </button>
            </div>
          </div>
          <span className="url-match">
            {u.matchType === "domain"
              ? t.detail.matchDomain
              : u.matchType === "origin"
                ? t.detail.matchOrigin
                : t.detail.matchExact}
          </span>
        </div>
      ))}
    </div>
  );
}

/** The websites group of an editor: an address and how it matches, per row. */
export function WebsitesEdit({ urls, onChange }: { urls: UrlRule[]; onChange: (next: UrlRule[]) => void }) {
  const { t } = useI18n();
  const matchLabels: Record<MatchType, string> = {
    domain: t.editor.matchDomain,
    origin: t.editor.matchOrigin,
    exact: t.editor.matchExact,
  };
  const update = (i: number, rule: Partial<UrlRule>) => onChange(urls.map((r, j) => (j === i ? { ...r, ...rule } : r)));
  return (
    <>
      <h3 className="group-title">{t.editor.websites}</h3>
      <div className="group">
        {urls.map((rule, i) => (
          <div className="row edit-row url-edit" key={i}>
            <input
              className="edit-input"
              value={rule.url}
              onChange={(e) => update(i, { url: e.target.value })}
              placeholder="github.com"
              spellCheck={false}
              autoCapitalize="off"
              inputMode="url"
              aria-label={t.editor.websiteN(i + 1)}
            />
            <span className="url-edit-controls">
              <span className="select-wrap">
                <select
                  value={rule.matchType}
                  onChange={(e) => update(i, { matchType: e.target.value as MatchType })}
                  aria-label={t.editor.matchHow(i + 1)}
                >
                  {matchTypes.map((m) => (
                    <option key={m} value={m}>
                      {matchLabels[m]}
                    </option>
                  ))}
                </select>
                <Icon name="chevronDown" size={14} className="select-chevron" />
              </span>
              <button
                type="button"
                className="icon-btn"
                onClick={() => onChange(urls.filter((_, j) => j !== i))}
                aria-label={t.editor.removeWebsiteN(i + 1)}
                title={t.editor.removeWebsite}
              >
                <Icon name="x" size={16} />
              </button>
            </span>
          </div>
        ))}
        <button
          type="button"
          className="row row-button add-row"
          onClick={() => onChange([...urls, { url: "", matchType: "domain" }])}
          disabled={urls.length >= 32}
        >
          <Icon name="plus" size={15} /> {t.editor.addWebsite}
        </button>
      </div>
    </>
  );
}

/**
 * The password row: a saved one is kept (dots, Change, Remove) until changed;
 * a typed one can be shown or generated. `saved` is whether there is one to keep.
 */
export function PasswordEdit({
  value,
  onChange,
  saved,
  label,
}: {
  value: SecretEdit;
  onChange: (next: SecretEdit) => void;
  saved: boolean;
  label: string;
}) {
  const { t } = useI18n();
  const toast = useToast();
  const [shown, setShown] = useState(false);

  async function generate() {
    try {
      const g = await api.generate({ length: 24, uppercase: true, lowercase: true, digits: true, symbols: true, avoidAmbiguous: false });
      onChange({ mode: "set", value: g.password });
      setShown(true);
    } catch (e) {
      toast(errorMessage(e, t, t.editor.generateFailed), "error");
    }
  }

  return (
    <div className="row edit-row">
      <span className="edit-label">{label}</span>
      {value.mode === "keep" ? (
        <div className="edit-secret">
          {/* Stands in for the password: may be cut short in a narrow pane. */}
          <span className="mono masked" data-truncate="">
            ••••••••••••
          </span>
          <span className="edit-secret-actions">
            <button type="button" className="btn btn-small" onClick={() => onChange({ mode: "set", value: "" })}>
              {t.editor.change}
            </button>
            <button type="button" className="btn btn-small btn-quiet-danger" onClick={() => onChange({ mode: "clear" })}>
              {t.common.remove}
            </button>
          </span>
        </div>
      ) : value.mode === "clear" ? (
        <div className="edit-secret">
          <span className="muted">{t.editor.passwordRemoved}</span>
          <span className="edit-secret-actions">
            <button type="button" className="btn btn-small" onClick={() => onChange(KEEP)}>
              {t.common.undo}
            </button>
          </span>
        </div>
      ) : value.mode === "set" ? (
        <div className="edit-secret">
          <input
            className="edit-input mono"
            type={shown ? "text" : "password"}
            value={value.value}
            onChange={(e) => onChange({ mode: "set", value: e.target.value })}
            placeholder={label}
            autoComplete="new-password"
            spellCheck={false}
            autoCapitalize="off"
            aria-label={label}
          />
          <span className="edit-secret-actions">
            <button
              type="button"
              className="icon-btn"
              onClick={() => setShown((s) => !s)}
              aria-label={shown ? t.common.hidePassword : t.common.showPassword}
              title={shown ? t.common.hidePassword : t.common.showPassword}
            >
              <Icon name={shown ? "eyeOff" : "eye"} size={16} />
            </button>
            <button type="button" className="btn btn-small" onClick={() => void generate()}>
              <Icon name="dice" size={15} /> {t.editor.generate}
            </button>
            {saved && (
              <button type="button" className="btn btn-small btn-quiet" onClick={() => onChange(KEEP)}>
                {t.common.cancel}
              </button>
            )}
          </span>
        </div>
      ) : null}
    </div>
  );
}

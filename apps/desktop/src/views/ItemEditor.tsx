import { useEffect, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { ItemInput, ItemOverview, ItemType, MatchType, SecretUpdate, UrlRule } from "../lib/types";
import { Icon } from "../components/Icon";

interface Props {
  itemType: ItemType;
  existing?: ItemOverview;
  /** Offline: saving would fail, so the control is disabled up front. */
  readOnly?: boolean;
  onCancel: () => void;
  onSaved: (item: ItemOverview) => void;
}

/** State of a secret the editor may not have read. */
type SecretEdit = { mode: "keep" } | { mode: "clear" } | { mode: "set"; value: string };

const KEEP: SecretEdit = { mode: "keep" };

function toUpdate(edit: SecretEdit): SecretUpdate {
  if (edit.mode === "set") return edit.value ? { op: "set", value: edit.value } : { op: "clear" };
  return { op: edit.mode };
}

const matchLabels: Record<MatchType, string> = {
  domain: "Whole site, any subdomain",
  origin: "This exact site",
  exact: "This exact page",
};

export function ItemEditor({ itemType, existing, readOnly, onCancel, onSaved }: Props) {
  const isNew = !existing;
  const [title, setTitle] = useState(existing?.title ?? "");
  const [username, setUsername] = useState(existing?.username ?? "");
  const [urls, setUrls] = useState<UrlRule[]>(
    existing?.urls.length ? existing.urls.map((u) => ({ ...u })) : [{ url: "", matchType: "domain" }],
  );
  const [password, setPassword] = useState<SecretEdit>(
    isNew || !existing.hasPassword ? { mode: "set", value: "" } : KEEP,
  );
  const [showPassword, setShowPassword] = useState(false);
  const [totp, setTotp] = useState<SecretEdit>(isNew || !existing.hasTotp ? { mode: "set", value: "" } : KEEP);
  // Notes/content are loaded (explicit edit action) so they can be edited in place.
  const [notes, setNotes] = useState<SecretEdit>(KEEP);
  const [notesText, setNotesText] = useState("");
  const [loadingText, setLoadingText] = useState(
    !isNew && (itemType === "secure_note" || existing.hasNotes),
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!existing || !loadingText) return;
    let cancelled = false;
    const field = itemType === "secure_note" ? "content" : "notes";
    api
      .reveal(existing.id, field)
      .then((t) => {
        if (cancelled) return;
        setNotesText(t);
        setLoadingText(false);
      })
      .catch(() => {
        if (cancelled) return;
        setError("Could not load the existing text.");
        setLoadingText(false);
      });
    return () => {
      cancelled = true;
    };
  }, [existing, itemType, loadingText]);

  async function generate() {
    try {
      const g = await api.generate({
        length: 24,
        uppercase: true,
        lowercase: true,
        digits: true,
        symbols: true,
        avoidAmbiguous: false,
      });
      setPassword({ mode: "set", value: g.password });
      setShowPassword(true);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "Could not generate a password.");
    }
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (saving || loadingText || readOnly) return;
    setSaving(true);
    setError(null);

    const textUpdate: SecretUpdate =
      notes.mode === "set" ? toUpdate(notes) : { op: "keep" };
    const input: ItemInput =
      itemType === "login"
        ? {
            itemType,
            title,
            username: username || null,
            urls: urls.filter((u) => u.url.trim()),
            password: toUpdate(password),
            totp: toUpdate(totp),
            notes: textUpdate,
          }
        : { itemType, title, content: textUpdate };

    try {
      const saved = existing ? await api.updateItem(existing.id, input) : await api.createItem(input);
      onSaved(saved);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not save the item.");
      setSaving(false);
    }
  }

  const heading = `${isNew ? "New" : "Edit"} ${itemType === "login" ? "login" : "secure note"}`;

  const saveDisabled = saving || loadingText || !title.trim() || readOnly;

  return (
    <form className="editor" onSubmit={submit}>
      <header className="editor-head" data-tauri-drag-region>
        <h2>{heading}</h2>
        <div className="editor-actions">
          <button type="button" className="btn btn-small" onClick={onCancel} disabled={saving}>
            Cancel
          </button>
          <button type="submit" className="btn btn-small btn-primary" disabled={saveDisabled}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </header>

      <div className="group">
        <label className="row edit-row">
          <span className="edit-label">Title</span>
          <input
            className="edit-input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={itemType === "login" ? "e.g. GitHub" : "e.g. Wi-Fi at home"}
            maxLength={256}
            autoFocus
            required
          />
        </label>

        {itemType === "login" && (
          <>
            <label className="row edit-row">
              <span className="edit-label">Username</span>
              <input
                className="edit-input"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Username or email"
                autoComplete="off"
                spellCheck={false}
                autoCapitalize="off"
                maxLength={512}
              />
            </label>

            <div className="row edit-row">
              <span className="edit-label">Password</span>
              {password.mode === "keep" ? (
                <div className="edit-secret">
                  <span className="mono masked">••••••••••••</span>
                  <button type="button" className="btn btn-small" onClick={() => setPassword({ mode: "set", value: "" })}>
                    Change
                  </button>
                  <button type="button" className="btn btn-small btn-quiet-danger" onClick={() => setPassword({ mode: "clear" })}>
                    Remove
                  </button>
                </div>
              ) : password.mode === "clear" ? (
                <div className="edit-secret">
                  <span className="muted">The password will be removed.</span>
                  <button type="button" className="btn btn-small" onClick={() => setPassword(KEEP)}>
                    Undo
                  </button>
                </div>
              ) : (
                <div className="edit-secret">
                  <input
                    className="edit-input mono"
                    type={showPassword ? "text" : "password"}
                    value={password.value}
                    onChange={(e) => setPassword({ mode: "set", value: e.target.value })}
                    placeholder="Password"
                    autoComplete="new-password"
                    spellCheck={false}
                    autoCapitalize="off"
                    aria-label="Password"
                  />
                  <button
                    type="button"
                    className="icon-btn"
                    onClick={() => setShowPassword((s) => !s)}
                    aria-label={showPassword ? "Hide password" : "Show password"}
                    title={showPassword ? "Hide password" : "Show password"}
                  >
                    <Icon name={showPassword ? "eyeOff" : "eye"} size={16} />
                  </button>
                  <button type="button" className="btn btn-small" onClick={() => void generate()}>
                    <Icon name="dice" size={15} /> Generate
                  </button>
                  {!isNew && existing.hasPassword && (
                    <button type="button" className="btn btn-small btn-quiet" onClick={() => setPassword(KEEP)}>
                      Cancel
                    </button>
                  )}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {itemType === "login" && (
        <>
          <h3 className="group-title">Websites</h3>
          <div className="group">
            {urls.map((rule, i) => (
              <div className="row edit-row url-edit" key={i}>
                <input
                  className="edit-input"
                  value={rule.url}
                  onChange={(e) =>
                    setUrls((list) => list.map((r, j) => (j === i ? { ...r, url: e.target.value } : r)))
                  }
                  placeholder="github.com"
                  spellCheck={false}
                  autoCapitalize="off"
                  inputMode="url"
                  aria-label={`Website ${i + 1}`}
                />
                <span className="select-wrap">
                  <select
                    value={rule.matchType}
                    onChange={(e) =>
                      setUrls((list) =>
                        list.map((r, j) => (j === i ? { ...r, matchType: e.target.value as MatchType } : r)),
                      )
                    }
                    aria-label={`How website ${i + 1} is matched`}
                  >
                    {(Object.keys(matchLabels) as MatchType[]).map((m) => (
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
                  onClick={() => setUrls((list) => list.filter((_, j) => j !== i))}
                  aria-label={`Remove website ${i + 1}`}
                  title="Remove website"
                >
                  <Icon name="x" size={16} />
                </button>
              </div>
            ))}
            <button
              type="button"
              className="row row-button add-row"
              onClick={() => setUrls((list) => [...list, { url: "", matchType: "domain" }])}
              disabled={urls.length >= 32}
            >
              <Icon name="plus" size={15} /> Add website
            </button>
          </div>

          <h3 className="group-title">One-time codes</h3>
          <div className="group">
            <div className="row edit-row">
              {totp.mode === "keep" ? (
                <div className="edit-secret">
                  <span className="muted">Set up.</span>
                  <button type="button" className="btn btn-small" onClick={() => setTotp({ mode: "set", value: "" })}>
                    Replace
                  </button>
                  <button type="button" className="btn btn-small btn-quiet-danger" onClick={() => setTotp({ mode: "clear" })}>
                    Remove
                  </button>
                </div>
              ) : totp.mode === "clear" ? (
                <div className="edit-secret">
                  <span className="muted">One-time codes will be removed.</span>
                  <button type="button" className="btn btn-small" onClick={() => setTotp(KEEP)}>
                    Undo
                  </button>
                </div>
              ) : (
                <input
                  className="edit-input mono"
                  type="password"
                  value={totp.value}
                  onChange={(e) => setTotp({ mode: "set", value: e.target.value })}
                  placeholder="Setup key or otpauth:// link"
                  autoComplete="off"
                  spellCheck={false}
                  autoCapitalize="off"
                  aria-label="One-time code setup key"
                />
              )}
            </div>
          </div>
        </>
      )}

      <h3 className="group-title">{itemType === "login" ? "Notes" : "Note"}</h3>
      <div className="group">
        <textarea
          className={`edit-area${itemType === "secure_note" ? " note-input" : ""}`}
          value={notesText}
          disabled={loadingText}
          placeholder={loadingText ? "Decrypting…" : itemType === "login" ? "Anything else worth keeping with this login" : undefined}
          aria-label={itemType === "login" ? "Notes" : "Note"}
          onChange={(e) => {
            setNotesText(e.target.value);
            setNotes({ mode: "set", value: e.target.value });
          }}
          spellCheck={false}
          rows={itemType === "login" ? 4 : 14}
        />
      </div>

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      {readOnly && (
        <p className="form-error" role="status">
          Offline — the vault is read-only until it reconnects.
        </p>
      )}
    </form>
  );
}

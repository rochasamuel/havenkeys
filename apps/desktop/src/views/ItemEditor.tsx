import { useEffect, useState, type FormEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { ItemInput, ItemOverview, ItemType, MatchType, SecretUpdate, UrlRule } from "../lib/types";
import { Icon } from "../components/Icon";

interface Props {
  itemType: ItemType;
  existing?: ItemOverview;
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
  domain: "Site and subdomains",
  origin: "This exact site",
  exact: "This exact page",
};

export function ItemEditor({ itemType, existing, onCancel, onSaved }: Props) {
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
    if (saving || loadingText) return;
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

  return (
    <form className="editor" onSubmit={submit}>
      <header className="editor-head">
        <h2>{heading}</h2>
      </header>

      <label className="control">
        <span>Title</span>
        <input
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
          <label className="control">
            <span>Username or email</span>
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              maxLength={512}
            />
          </label>

          <div className="control">
            <span>Password</span>
            {password.mode === "keep" ? (
              <div className="secret-keep">
                <span className="mono masked">••••••••••••</span>
                <button type="button" className="btn btn-small" onClick={() => setPassword({ mode: "set", value: "" })}>
                  Change
                </button>
                <button type="button" className="btn btn-small btn-quiet-danger" onClick={() => setPassword({ mode: "clear" })}>
                  Remove
                </button>
              </div>
            ) : password.mode === "clear" ? (
              <div className="secret-keep">
                <span className="muted">The password will be removed.</span>
                <button type="button" className="btn btn-small" onClick={() => setPassword(KEEP)}>
                  Undo
                </button>
              </div>
            ) : (
              <div className="input-row">
                <input
                  className="mono"
                  type={showPassword ? "text" : "password"}
                  value={password.value}
                  onChange={(e) => setPassword({ mode: "set", value: e.target.value })}
                  autoComplete="new-password"
                  spellCheck={false}
                  autoCapitalize="off"
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
                  <button type="button" className="btn btn-small" onClick={() => setPassword(KEEP)}>
                    Cancel
                  </button>
                )}
              </div>
            )}
          </div>

          <div className="control">
            <span>Websites</span>
            <div className="url-rows">
              {urls.map((rule, i) => (
                <div className="input-row" key={i}>
                  <input
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
                className="btn btn-small btn-ghost"
                onClick={() => setUrls((list) => [...list, { url: "", matchType: "domain" }])}
                disabled={urls.length >= 32}
              >
                <Icon name="plus" size={15} /> Add website
              </button>
            </div>
          </div>

          <div className="control">
            <span>One-time codes (TOTP)</span>
            {totp.mode === "keep" ? (
              <div className="secret-keep">
                <span className="muted">Set up.</span>
                <button type="button" className="btn btn-small" onClick={() => setTotp({ mode: "set", value: "" })}>
                  Replace
                </button>
                <button type="button" className="btn btn-small btn-quiet-danger" onClick={() => setTotp({ mode: "clear" })}>
                  Remove
                </button>
              </div>
            ) : totp.mode === "clear" ? (
              <div className="secret-keep">
                <span className="muted">One-time codes will be removed.</span>
                <button type="button" className="btn btn-small" onClick={() => setTotp(KEEP)}>
                  Undo
                </button>
              </div>
            ) : (
              <input
                className="mono"
                type="password"
                value={totp.value}
                onChange={(e) => setTotp({ mode: "set", value: e.target.value })}
                placeholder="Setup key or otpauth:// link"
                autoComplete="off"
                spellCheck={false}
                autoCapitalize="off"
              />
            )}
          </div>
        </>
      )}

      <label className="control">
        <span>{itemType === "login" ? "Notes" : "Note"}</span>
        <textarea
          className={itemType === "secure_note" ? "note-input" : undefined}
          value={notesText}
          disabled={loadingText}
          placeholder={loadingText ? "Decrypting…" : undefined}
          onChange={(e) => {
            setNotesText(e.target.value);
            setNotes({ mode: "set", value: e.target.value });
          }}
          spellCheck={false}
          rows={itemType === "login" ? 4 : 14}
        />
      </label>

      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}

      <footer className="editor-foot">
        <button type="button" className="btn" onClick={onCancel} disabled={saving}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" disabled={saving || loadingText || !title.trim()}>
          {saving ? "Saving…" : "Save"}
        </button>
      </footer>
    </form>
  );
}

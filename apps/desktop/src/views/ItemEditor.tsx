import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import type { ItemInput, ItemOverview, ItemType, MatchType, SecretUpdate, SignInWith, UrlRule } from "../lib/types";
import { EMPTY, KEEP, toUpdate, type SecretEdit } from "../lib/secretEdit";
import { fromViews, sectionsInput, type EditSection } from "../lib/customFields";
import { isDirty, type EditorSnapshot } from "../lib/openItem";
import { shouldCollapse } from "../lib/sso";
import { Icon } from "../components/Icon";
import { TagsEditor, type TagsEditorHandle } from "../components/TagsEditor";
import { SsoPicker } from "../components/SsoPicker";
import { Switch } from "../components/Switch";
import { TotpEdit } from "../components/TotpEdit";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { CustomFieldsEditor } from "./CustomFieldsEditor";

interface Props {
  itemType: ItemType;
  existing?: ItemOverview;
  /** Offline: saving would fail, so the control is disabled up front. */
  readOnly?: boolean;
  onCancel: () => void;
  onSaved: (item: ItemOverview) => void;
  onDirtyChange?: (dirty: boolean) => void;
}

const matchTypes: MatchType[] = ["domain", "origin", "exact"];

export function ItemEditor({ itemType, existing, readOnly, onCancel, onSaved, onDirtyChange }: Props) {
  const { t } = useI18n();
  const matchLabels: Record<MatchType, string> = {
    domain: t.editor.matchDomain,
    origin: t.editor.matchOrigin,
    exact: t.editor.matchExact,
  };
  const isNew = !existing;
  const [title, setTitle] = useState(existing?.title ?? "");
  const [username, setUsername] = useState(existing?.username ?? "");
  const [urls, setUrls] = useState<UrlRule[]>(
    existing?.urls.length ? existing.urls.map((u) => ({ ...u })) : [{ url: "", matchType: "domain" }],
  );
  const [password, setPassword] = useState<SecretEdit>(isNew || !existing.hasPassword ? EMPTY : KEEP);
  const [showPassword, setShowPassword] = useState(false);
  const [totp, setTotp] = useState<SecretEdit>(isNew || !existing.hasTotp ? EMPTY : KEEP);
  // A login's custom fields. `null` while an existing login's are loading
  // or failed to load: nothing renders and the save sends no `sections`,
  // so Rust keeps them as they are.
  const [sections, setSections] = useState<EditSection[] | null>(itemType === "login" && isNew ? [] : null);
  // Notes/content are loaded (explicit edit action) so they can be edited in place.
  const [notes, setNotes] = useState<SecretEdit>(KEEP);
  const [notesText, setNotesText] = useState("");
  const [loadingText, setLoadingText] = useState(
    !isNew && (itemType === "secure_note" || existing.hasNotes),
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [autoSignIn, setAutoSignIn] = useState(existing?.autoSignIn ?? true);
  const [tags, setTags] = useState<string[]>(existing?.tags ?? []);
  const tagsRow = useRef<TagsEditorHandle>(null);
  const [globalAutoSignIn, setGlobalAutoSignIn] = useState(true);
  const [signIn, setSignIn] = useState<SignInWith | null>(existing?.signInWith ?? null);
  const [passwordOpen, setPasswordOpen] = useState(!existing?.signInWith || existing.hasPassword);

  const snapshot: EditorSnapshot = { title, username, urls, password, totp, notes, autoSignIn, tags, signInWith: signIn, sections };
  const [initialSnapshot, setInitialSnapshot] = useState(snapshot);
  const dirty = isDirty(initialSnapshot, snapshot);
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);

  useEffect(() => {
    let live = true;
    api
      .getSettings()
      .then((s) => live && setGlobalAutoSignIn(s.autoSignIn))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    if (!existing || !loadingText) return;
    let cancelled = false;
    const field = itemType === "secure_note" ? "content" : "notes";
    api
      .reveal(existing.id, field)
      .then((text) => {
        if (cancelled) return;
        setNotesText(text);
        setLoadingText(false);
      })
      .catch(() => {
        if (cancelled) return;
        setError(t.editor.loadTextFailed);
        setLoadingText(false);
      });
    return () => {
      cancelled = true;
    };
    // `t` is left out: a language change must not decrypt the text again.
  }, [existing, itemType, loadingText]);

  // Keyed on the id, not the overview object: a list refresh (sync) hands
  // the editor a new `existing` and must not reload over the user's edits.
  const existingId = existing?.id;
  useEffect(() => {
    if (!existingId || itemType !== "login") return;
    let cancelled = false;
    api
      .loginFields(existingId)
      .then((views) => {
        if (cancelled) return;
        const loaded = fromViews(views);
        setSections(loaded);
        setInitialSnapshot((s) => ({ ...s, sections: loaded }));
      })
      .catch(() => {
        if (!cancelled) setError(t.fields.loadFailed);
      });
    return () => {
      cancelled = true;
    };
    // `t` is left out, as for the notes: a language change must not reload.
  }, [existingId, itemType]);

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
      setError(errorMessage(e, t, t.editor.generateFailed));
    }
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (saving || loadingText || readOnly) return;
    // A tag still being typed is saved too; one Rust would refuse keeps the editor open.
    const finalTags = tagsRow.current ? tagsRow.current.commit() : tags;
    if (finalTags === null) return;
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
            autoSignIn,
            signInWith: signIn ? { provider: signIn.provider, account: signIn.account?.trim() || null } : null,
            sections: sectionsInput(sections, t.fields.types),
            tags: finalTags,
          }
        : { itemType, title, content: textUpdate, tags: finalTags };

    try {
      const saved = existing ? await api.updateItem(existing.id, input) : await api.createItem(input);
      onSaved(saved);
    } catch (err) {
      setError(errorMessage(err, t, t.editor.saveFailed));
      setSaving(false);
    }
  }

  const heading =
    itemType === "login"
      ? isNew
        ? t.editor.newLogin
        : t.editor.editLogin
      : isNew
        ? t.editor.newNote
        : t.editor.editNote;

  const saveDisabled = saving || loadingText || !title.trim() || readOnly;

  return (
    <form className="editor" onSubmit={submit}>
      <header className="editor-head" data-tauri-drag-region>
        <h2>{heading}</h2>
        <div className="editor-actions">
          <button type="button" className="btn btn-small" onClick={onCancel} disabled={saving}>
            {t.common.cancel}
          </button>
          <button type="submit" className="btn btn-small btn-primary" disabled={saveDisabled}>
            {saving ? t.common.saving : t.common.save}
          </button>
        </div>
      </header>

      <div className={itemType === "login" ? "group sso-group" : "group"}>
        <label className="row edit-row">
          <span className="edit-label">{t.editor.title}</span>
          <input
            className="edit-input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={itemType === "login" ? t.editor.titlePlaceholderLogin : t.editor.titlePlaceholderNote}
            maxLength={256}
            autoFocus
            required
          />
        </label>

        {itemType === "login" && (
          <>
            <div className="row edit-row sso-row">
              <span className="edit-label">{t.editor.signInWith}</span>
              <SsoPicker
                value={signIn}
                onChange={(next) => {
                  setSignIn(next);
                  if (!next) setPasswordOpen(true);
                  else if (shouldCollapse(username, password, existing?.hasPassword ?? false)) setPasswordOpen(false);
                }}
              />
            </div>
            {signIn && (
              <label className="row edit-row">
                <span className="edit-label">{t.editor.account}</span>
                <input
                  className="edit-input"
                  value={signIn.account ?? ""}
                  onChange={(e) => setSignIn({ provider: signIn.provider, account: e.target.value })}
                  placeholder={t.editor.accountPlaceholder}
                  autoComplete="off"
                  spellCheck={false}
                  autoCapitalize="off"
                  maxLength={254}
                />
              </label>
            )}
            {passwordOpen ? (
              <>
                <label className="row edit-row">
                  <span className="edit-label">{t.common.username}</span>
                  <input
                    className="edit-input"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    placeholder={t.editor.usernamePlaceholder}
                    autoComplete="off"
                    spellCheck={false}
                    autoCapitalize="off"
                    maxLength={512}
                  />
                </label>

                <div className="row edit-row">
                  <span className="edit-label">{t.common.password}</span>
                  {password.mode === "keep" ? (
                    <div className="edit-secret">
                      {/* Stands in for the password: may be cut short in a narrow pane. */}
                      <span className="mono masked" data-truncate="">
                        ••••••••••••
                      </span>
                      <span className="edit-secret-actions">
                        <button type="button" className="btn btn-small" onClick={() => setPassword({ mode: "set", value: "" })}>
                          {t.editor.change}
                        </button>
                        <button type="button" className="btn btn-small btn-quiet-danger" onClick={() => setPassword({ mode: "clear" })}>
                          {t.common.remove}
                        </button>
                      </span>
                    </div>
                  ) : password.mode === "clear" ? (
                    <div className="edit-secret">
                      <span className="muted">{t.editor.passwordRemoved}</span>
                      <span className="edit-secret-actions">
                        <button type="button" className="btn btn-small" onClick={() => setPassword(KEEP)}>
                          {t.common.undo}
                        </button>
                      </span>
                    </div>
                  ) : password.mode === "set" ? (
                    <div className="edit-secret">
                      <input
                        className="edit-input mono"
                        type={showPassword ? "text" : "password"}
                        value={password.value}
                        onChange={(e) => setPassword({ mode: "set", value: e.target.value })}
                        placeholder={t.common.password}
                        autoComplete="new-password"
                        spellCheck={false}
                        autoCapitalize="off"
                        aria-label={t.common.password}
                      />
                      <span className="edit-secret-actions">
                        <button
                          type="button"
                          className="icon-btn"
                          onClick={() => setShowPassword((s) => !s)}
                          aria-label={showPassword ? t.common.hidePassword : t.common.showPassword}
                          title={showPassword ? t.common.hidePassword : t.common.showPassword}
                        >
                          <Icon name={showPassword ? "eyeOff" : "eye"} size={16} />
                        </button>
                        <button type="button" className="btn btn-small" onClick={() => void generate()}>
                          <Icon name="dice" size={15} /> {t.editor.generate}
                        </button>
                        {!isNew && existing.hasPassword && (
                          <button type="button" className="btn btn-small btn-quiet" onClick={() => setPassword(KEEP)}>
                            {t.common.cancel}
                          </button>
                        )}
                      </span>
                    </div>
                  ) : null}
                </div>
              </>
            ) : (
              <button type="button" className="row row-button add-row" onClick={() => setPasswordOpen(true)}>
                <Icon name="plus" size={15} /> {t.editor.alsoPassword}
              </button>
            )}
          </>
        )}
      </div>

      <div className="group">
        <TagsEditor value={tags} onChange={setTags} disabled={readOnly || saving} handle={tagsRow} />
      </div>

      {itemType === "login" && (
        <>
          <h3 className="group-title">{t.editor.websites}</h3>
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
                  aria-label={t.editor.websiteN(i + 1)}
                />
                <span className="url-edit-controls">
                  <span className="select-wrap">
                    <select
                      value={rule.matchType}
                      onChange={(e) =>
                        setUrls((list) =>
                          list.map((r, j) => (j === i ? { ...r, matchType: e.target.value as MatchType } : r)),
                        )
                      }
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
                    onClick={() => setUrls((list) => list.filter((_, j) => j !== i))}
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
              onClick={() => setUrls((list) => [...list, { url: "", matchType: "domain" }])}
              disabled={urls.length >= 32}
            >
              <Icon name="plus" size={15} /> {t.editor.addWebsite}
            </button>
          </div>

          <h3 className="group-title">{t.editor.oneTimeCodes}</h3>
          <div className="group">
            <TotpEdit
              edit={totp}
              onChange={setTotp}
              disabled={saving || !!readOnly}
              ariaLabel={t.editor.totpLabel}
              onError={setError}
            />
          </div>

          {sections !== null && (
            <CustomFieldsEditor
              itemId={existingId}
              sections={sections}
              onChange={setSections}
              disabled={saving || !!readOnly}
              onError={setError}
            />
          )}

          <h3 className="group-title">{t.editor.browser}</h3>
          <div className="group">
            <div className="row">
              <span className="row-label-inline">{t.editor.autoSignIn}</span>
              <Switch
                label={t.editor.autoSignIn}
                checked={autoSignIn && globalAutoSignIn}
                disabled={!globalAutoSignIn || readOnly}
                onChange={setAutoSignIn}
              />
            </div>
          </div>
          {!globalAutoSignIn && <p className="group-note">{t.editor.offInSettings}</p>}
        </>
      )}

      <h3 className="group-title">{itemType === "login" ? t.editor.notes : t.editor.note}</h3>
      <div className="group">
        <textarea
          className={`edit-area${itemType === "secure_note" ? " note-input" : ""}`}
          value={notesText}
          disabled={loadingText}
          placeholder={loadingText ? t.common.decrypting : itemType === "login" ? t.editor.notesPlaceholder : undefined}
          aria-label={itemType === "login" ? t.editor.notes : t.editor.note}
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
          {t.common.offlineReadOnly}
        </p>
      )}
    </form>
  );
}

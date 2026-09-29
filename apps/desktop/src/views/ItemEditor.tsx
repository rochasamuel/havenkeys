import { useEffect, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import type { ItemInput, ItemOverview, ItemType, MatchType, ScannedTotp, SecretUpdate, SignInWith, SsoProvider, UrlRule } from "../lib/types";
import { EMPTY, KEEP, canScan, scanLabel, toUpdate, type SecretEdit } from "../lib/secretEdit";
import { isDirty, type EditorSnapshot } from "../lib/openItem";
import { PROVIDER_NAMES, PROVIDER_ORDER, shouldCollapse } from "../lib/sso";
import { Icon } from "../components/Icon";
import { Switch } from "../components/Switch";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

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
  const [scanning, setScanning] = useState(false);
  const [scanChoices, setScanChoices] = useState<ScannedTotp[] | null>(null);
  // Notes/content are loaded (explicit edit action) so they can be edited in place.
  const [notes, setNotes] = useState<SecretEdit>(KEEP);
  const [notesText, setNotesText] = useState("");
  const [loadingText, setLoadingText] = useState(
    !isNew && (itemType === "secure_note" || existing.hasNotes),
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [autoSignIn, setAutoSignIn] = useState(existing?.autoSignIn ?? true);
  const [globalAutoSignIn, setGlobalAutoSignIn] = useState(true);
  const [signIn, setSignIn] = useState<SignInWith | null>(existing?.signInWith ?? null);
  const [passwordOpen, setPasswordOpen] = useState(!existing?.signInWith || existing.hasPassword);

  const snapshot: EditorSnapshot = { title, username, urls, password, totp, notes, autoSignIn, signInWith: signIn };
  const [initialSnapshot] = useState(snapshot);
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

  async function scanQr() {
    setScanning(true);
    setError(null);
    setScanChoices(null);
    try {
      const found = await api.scanTotpQr();
      if (found.length === 1) pickScan(found[0]!);
      else setScanChoices(found);
    } catch (e) {
      setError(errorMessage(e, t, t.editor.scanFailed));
    } finally {
      setScanning(false);
    }
  }

  function pickScan(code: ScannedTotp) {
    setScanChoices(null);
    setTotp({ mode: "scanned", token: code.token, label: scanLabel(code, t.editor.scanUnnamed) });
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
            autoSignIn,
            signInWith: signIn ? { provider: signIn.provider, account: signIn.account?.trim() || null } : null,
          }
        : { itemType, title, content: textUpdate };

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

      <div className="group">
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
            <div className="row edit-row">
              <span className="edit-label">{t.editor.signInWith}</span>
              <span className="select-wrap">
                <select
                  value={signIn?.provider ?? ""}
                  aria-label={t.editor.providerPick}
                  onChange={(e) => {
                    const p = e.target.value as SsoProvider | "";
                    setSignIn(p ? { provider: p, account: signIn?.account ?? null } : null);
                    if (!p) setPasswordOpen(true);
                    else if (shouldCollapse(username, password, existing?.hasPassword ?? false)) setPasswordOpen(false);
                  }}
                >
                  <option value="">{t.editor.providerNone}</option>
                  {PROVIDER_ORDER.map((p) => (
                    <option key={p} value={p}>{PROVIDER_NAMES[p]}</option>
                  ))}
                </select>
                <Icon name="chevronDown" size={14} className="select-chevron" />
              </span>
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
            <div className="row edit-row">
              {totp.mode === "keep" ? (
                <div className="edit-secret">
                  <span className="muted">{t.editor.setUp}</span>
                  <span className="edit-secret-actions">
                    <button type="button" className="btn btn-small" onClick={() => setTotp(EMPTY)}>
                      {t.editor.replace}
                    </button>
                    <button type="button" className="btn btn-small btn-quiet-danger" onClick={() => setTotp({ mode: "clear" })}>
                      {t.common.remove}
                    </button>
                  </span>
                </div>
              ) : totp.mode === "clear" ? (
                <div className="edit-secret">
                  <span className="muted">{t.editor.codesRemoved}</span>
                  <span className="edit-secret-actions">
                    <button type="button" className="btn btn-small" onClick={() => setTotp(KEEP)}>
                      {t.common.undo}
                    </button>
                  </span>
                </div>
              ) : totp.mode === "scanned" ? (
                <div className="edit-secret">
                  <span className="scanned-code">
                    <Icon name="check" size={15} /> {t.editor.scanned(totp.label)}
                  </span>
                  <span className="edit-secret-actions">
                    <button type="button" className="btn btn-small" onClick={() => setTotp(EMPTY)}>
                      {t.common.undo}
                    </button>
                  </span>
                </div>
              ) : (
                <div className="edit-secret">
                  <input
                    className="edit-input mono"
                    type="password"
                    value={totp.value}
                    onChange={(e) => {
                      setTotp({ mode: "set", value: e.target.value });
                      setScanChoices(null);
                    }}
                    placeholder={t.editor.totpPlaceholder}
                    autoComplete="off"
                    spellCheck={false}
                    autoCapitalize="off"
                    aria-label={t.editor.totpLabel}
                  />
                  {canScan(totp) && (
                    <span className="edit-secret-actions">
                      <button
                        type="button"
                        className="icon-btn"
                        onClick={() => void scanQr()}
                        disabled={scanning || saving || readOnly}
                        aria-label={scanning ? t.editor.scanning : t.editor.scanQr}
                        title={scanning ? t.editor.scanning : t.editor.scanQr}
                      >
                        {scanning ? <span className="spinner" aria-hidden="true" /> : <Icon name="qr" size={16} />}
                      </button>
                    </span>
                  )}
                </div>
              )}
            </div>
            {scanChoices && scanChoices.length > 1 && (
              <div className="row scan-choices">
                <span className="muted">{t.editor.scanPick}</span>
                {scanChoices.map((code) => (
                  <button key={code.token} type="button" className="btn btn-small" onClick={() => pickScan(code)}>
                    {scanLabel(code, t.editor.scanUnnamed)}
                  </button>
                ))}
              </div>
            )}
          </div>

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

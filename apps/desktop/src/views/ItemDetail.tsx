import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { CopyField, HealthReport, ItemOverview, PasskeyInfo } from "../lib/types";
import { checksFor } from "../lib/health";
import { formatDate, monogram, primaryHost } from "../lib/format";
import { useRevealedSecret } from "../lib/hooks";
import { CopyButton } from "../components/CopyButton";
import { Field, IconButton, TotpField } from "../components/Field";
import { HealthChips } from "../components/HealthChips";
import { Icon } from "../components/Icon";
import { SsoRow } from "../components/SsoRow";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";
import { CustomSections } from "./CustomSections";

interface Props {
  item: ItemOverview;
  /** Bumped when the vault reloads its items (see SsoRow). */
  revision: number;
  /** The vault health report, for this login's chips under its title. */
  health?: HealthReport | null;
  /** Offline: editing or deleting would fail, so the controls are disabled up front. */
  readOnly: boolean;
  onEdit: () => void;
  /** Open the vault filtered by this tag. */
  onTag?: (tag: string) => void;
  onDelete: () => void;
  onOpen: (id: string) => void;
}

function useCopy(itemId: string) {
  const toast = useToast();
  const { t } = useI18n();
  return useCallback(
    async (field: CopyField) => {
      try {
        const r = await api.copy(itemId, field);
        toast(t.detail.copied[field](r.clearAfterSeconds));
        return true;
      } catch (e) {
        toast(errorMessage(e, t, t.common.couldNotCopy), "error");
        return false;
      }
    },
    [itemId, toast, t],
  );
}

/** One previous password, hidden until asked for. */
function PreviousPassword({ itemId, index, replacedAt }: { itemId: string; index: number; replacedAt: number }) {
  const toast = useToast();
  const { t, dateLocale } = useI18n();
  const secret = useRevealedSecret(useCallback(() => api.revealPreviousPassword(itemId, index), [itemId, index]));
  const toggle = () =>
    secret.value === null
      ? void secret.reveal().catch((e) => toast(errorMessage(e, t, t.common.couldNotReveal), "error"))
      : secret.hide();
  return (
    <li className="history-row">
      {secret.value === null ? (
        <span className="mono masked" aria-label={t.common.hiddenPassword} data-truncate="">
          ••••••••••••
        </span>
      ) : (
        <span className="mono selectable revealed">{secret.value}</span>
      )}
      <span className="muted">{t.detail.replaced(formatDate(replacedAt, dateLocale))}</span>
      <IconButton
        icon={secret.value === null ? "eye" : "eyeOff"}
        label={secret.value === null ? t.detail.showPrevious : t.detail.hidePrevious}
        onClick={toggle}
      />
    </li>
  );
}

/** Passwords this login used before. Loaded only when the user asks. */
function PasswordHistory({ itemId }: { itemId: string }) {
  const toast = useToast();
  const { t } = useI18n();
  const [dates, setDates] = useState<number[] | null>(null);
  const load = () =>
    api.passwordHistory(itemId).then(setDates, (e) =>
      toast(errorMessage(e, t, t.detail.historyFailed), "error"),
    );
  if (dates === null) {
    return (
      <button className="row row-button" onClick={() => void load()}>
        <span className="row-label-inline">{t.detail.passwordHistory}</span>
        <Icon name="chevronDown" size={15} className="row-chevron" />
      </button>
    );
  }
  if (dates.length === 0) {
    return (
      <div className="row">
        <span className="row-label-inline">{t.detail.passwordHistory}</span>
        <span className="muted">{t.detail.noPrevious}</span>
      </div>
    );
  }
  return (
    <Field label={t.detail.previousPasswords}>
      <ul className="history-list">
        {dates.map((d, i) => (
          <PreviousPassword key={`${i}-${d}`} itemId={itemId} index={i} replacedAt={d} />
        ))}
      </ul>
    </Field>
  );
}

/**
 * Passkeys saved on this login. Private keys never leave the core. A passkey's
 * account name is left out when it repeats the login's username shown above.
 */
function Passkeys({ itemId, username, readOnly }: { itemId: string; username: string | null; readOnly: boolean }) {
  const toast = useToast();
  const { t, dateLocale } = useI18n();
  const [list, setList] = useState<PasskeyInfo[] | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api.listPasskeys(itemId).then(
      (l) => !cancelled && setList(l),
      (e) => !cancelled && toast(errorMessage(e, t, t.detail.passkeysFailed), "error"),
    );
    return () => {
      cancelled = true;
    };
    // `t` is left out: a language change must not reload the list.
  }, [itemId, toast]);

  const remove = async (credentialId: string) => {
    try {
      await api.deletePasskey(itemId, credentialId);
      setList((l) => l?.filter((p) => p.credentialId !== credentialId) ?? null);
      setConfirming(null);
      toast(t.detail.passkeyDeleted);
    } catch (e) {
      toast(errorMessage(e, t, t.detail.passkeyDeleteFailed), "error");
    }
  };

  if (list === null || list.length === 0) return null;
  return (
    <Field label={t.detail.passkeys}>
      <ul className="history-list">
        {list.map((p) => (
          <li className="history-row" key={p.credentialId}>
            <Icon name="key" size={15} />
            <span className="selectable">{p.rpId}</span>
            <span className="muted">
              {username && p.userName === username
                ? t.detail.passkeySaved(formatDate(p.createdAt, dateLocale))
                : `${p.userName || p.displayName || t.detail.noAccountName} · ${t.detail.passkeySaved(formatDate(p.createdAt, dateLocale))}`}
            </span>
            {confirming === p.credentialId ? (
              <span className="confirm">
                <span>{t.detail.loseAccess(p.rpId)}</span>
                <button className="btn btn-small" onClick={() => setConfirming(null)}>
                  {t.common.keep}
                </button>
                <button className="btn btn-small btn-danger" onClick={() => void remove(p.credentialId)}>
                  {t.common.delete}
                </button>
              </span>
            ) : (
              <IconButton icon="trash" label={t.detail.deletePasskey} onClick={() => setConfirming(p.credentialId)} disabled={readOnly} />
            )}
          </li>
        ))}
      </ul>
    </Field>
  );
}

export function ItemDetail({ item, revision, health, readOnly, onTag, onEdit, onDelete, onOpen }: Props) {
  const toast = useToast();
  const { t, dateLocale } = useI18n();
  const copy = useCopy(item.id);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const password = useRevealedSecret(useCallback(() => api.reveal(item.id, "password"), [item.id]));
  const notes = useRevealedSecret(useCallback(() => api.reveal(item.id, "notes"), [item.id]), 120_000);
  const [content, setContent] = useState<string | null>(null);

  // Opening a secure note is the explicit action that decrypts its body.
  useEffect(() => {
    if (item.itemType !== "secure_note") return;
    let cancelled = false;
    api
      .reveal(item.id, "content")
      .then((c) => !cancelled && setContent(c))
      .catch((e) => !cancelled && toast(errorMessage(e, t, t.detail.noteFailed), "error"));
    return () => {
      cancelled = true;
      setContent(null);
    };
    // `t` is left out: a language change must not decrypt the note again.
  }, [item.id, item.itemType, toast]);

  const onRevealError = (e: unknown) => toast(errorMessage(e, t, t.common.couldNotReveal), "error");

  const host = primaryHost(item);

  return (
    <article className="item">
      <header className="item-head" data-tauri-drag-region>
        <span className={`avatar avatar-lg avatar-${item.itemType}`} aria-hidden="true">
          {item.itemType === "secure_note" ? <Icon name="note" size={26} /> : monogram(item.title)}
        </span>
        <div className="item-head-text">
          <h2 className="item-title">{item.title}</h2>
          <p className="item-kind">{item.itemType === "login" ? (host ?? t.common.login) : t.common.secureNote}</p>
        </div>
        <div className="item-head-actions">
          <button className="btn btn-small" onClick={onEdit} disabled={readOnly}>
            <Icon name="edit" size={15} /> {t.common.edit}
          </button>
        </div>
        {health && <HealthChips checks={checksFor(health, item.id)} report={health} itemId={item.id} />}
      </header>

      {item.tags.length > 0 && (
        <div className="detail-tags" role="group" aria-label={t.detail.tags}>
          {item.tags.map((name) => (
            <button key={name} type="button" className="chip tag-chip" onClick={() => onTag?.(name)}>
              {name}
            </button>
          ))}
        </div>
      )}

      {item.itemType === "login" && (
        <>
          <div className="group">
            {item.signInWith && <SsoRow item={item} revision={revision} onOpen={onOpen} />}

            {item.username && (
              <Field
                label={t.common.username}
                actions={<CopyButton label={t.detail.copyUsername} onCopy={() => copy("username")} />}
              >
                <span className="selectable">{item.username}</span>
              </Field>
            )}

            {item.hasPassword && (
              <Field
                label={t.common.password}
                actions={
                  <>
                    <IconButton
                      icon={password.value === null ? "eye" : "eyeOff"}
                      label={password.value === null ? t.common.showPassword : t.common.hidePassword}
                      onClick={() =>
                        password.value === null ? void password.reveal().catch(onRevealError) : password.hide()
                      }
                    />
                    <CopyButton label={t.detail.copyPassword} onCopy={() => copy("password")} />
                  </>
                }
              >
                <span className="secret" data-revealed={password.value !== null}>
                  {password.value === null ? (
                    <span className="mono masked" aria-label={t.common.hiddenPassword}>
                      ••••••••••••
                    </span>
                  ) : (
                    <span className="mono selectable revealed">{password.value}</span>
                  )}
                </span>
              </Field>
            )}

            {item.hasTotp && (
              <TotpField
                label={t.detail.oneTimeCode}
                load={() => api.totp(item.id)}
                loadKey={item.id}
                actions={<CopyButton label={t.detail.copyOneTimeCode} onCopy={() => copy("totp")} />}
              />
            )}
          </div>

          {item.hasPasskey && (
            <div className="group">
              <Passkeys itemId={item.id} username={item.username} readOnly={readOnly} />
            </div>
          )}

          {item.urls.length > 0 && (
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
          )}

          <CustomSections itemId={item.id} updatedAt={item.updatedAt} />

          {item.hasNotes && (
            <div className="group">
              <Field
                label={t.detail.notes}
                actions={
                  <IconButton
                    icon={notes.value === null ? "eye" : "eyeOff"}
                    label={notes.value === null ? t.detail.showNotes : t.detail.hideNotes}
                    onClick={() => (notes.value === null ? void notes.reveal().catch(onRevealError) : notes.hide())}
                  />
                }
              >
                {notes.value === null ? (
                  <span className="muted">{t.detail.hidden}</span>
                ) : (
                  <p className="note-body selectable">{notes.value}</p>
                )}
              </Field>
            </div>
          )}

          <div className="group group-history">
            <PasswordHistory itemId={item.id} />
          </div>
        </>
      )}

      {item.itemType === "secure_note" && (
        <div className="note-sheet">
          {content === null ? (
            <p className="muted">{t.common.decrypting}</p>
          ) : (
            <p className="note-body selectable">{content}</p>
          )}
        </div>
      )}

      <footer className="item-foot">
        <p className="muted">
          {t.detail.dates(formatDate(item.createdAt, dateLocale), formatDate(item.updatedAt, dateLocale))}
        </p>
        {confirmDelete ? (
          <div className="confirm">
            <span>
              {t.detail.confirmDelete(item.title)}
              {item.hasPasskey && t.detail.passkeyWarning}
            </span>
            <button className="btn btn-small" onClick={() => setConfirmDelete(false)}>
              {t.common.keep}
            </button>
            <button className="btn btn-small btn-danger" onClick={onDelete}>
              {t.common.delete}
            </button>
          </div>
        ) : (
          <button
            className="btn btn-small btn-quiet-danger"
            onClick={() => setConfirmDelete(true)}
            disabled={readOnly}
          >
            <Icon name="trash" size={15} /> {t.common.delete}
          </button>
        )}
      </footer>
    </article>
  );
}

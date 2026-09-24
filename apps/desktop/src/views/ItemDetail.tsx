import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, ApiError } from "../lib/api";
import type { CopyField, ItemOverview, PasskeyInfo } from "../lib/types";
import { formatDate, groupCode, monogram, primaryHost } from "../lib/format";
import { useRevealedSecret, useTotp } from "../lib/hooks";
import { CopyButton } from "../components/CopyButton";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";

interface Props {
  item: ItemOverview;
  /** Offline: editing or deleting would fail, so the controls are disabled up front. */
  readOnly: boolean;
  onEdit: () => void;
  onDelete: () => void;
}

function useCopy(itemId: string) {
  const toast = useToast();
  return useCallback(
    async (field: CopyField, label: string) => {
      try {
        const r = await api.copy(itemId, field);
        toast(`${label} copied. The clipboard clears in ${r.clearAfterSeconds} s.`);
        return true;
      } catch (e) {
        toast(e instanceof ApiError ? e.message : "Could not copy.", "error");
        return false;
      }
    },
    [itemId, toast],
  );
}

function Field({ label, children, actions }: { label: string; children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="row">
      <div className="row-main">
        <div className="row-label">{label}</div>
        <div className="row-value">{children}</div>
      </div>
      {actions && <div className="row-actions-inline">{actions}</div>}
    </div>
  );
}

function IconButton({
  icon,
  label,
  onClick,
  disabled,
}: {
  icon: Parameters<typeof Icon>[0]["name"];
  label: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button className="icon-btn" onClick={onClick} title={label} aria-label={label} disabled={disabled}>
      <Icon name={icon} size={16} />
    </button>
  );
}

function TotpField({ item, onCopy }: { item: ItemOverview; onCopy: () => Promise<boolean> }) {
  const { code, remaining, failed } = useTotp(item.id, true);
  const period = code?.period ?? 30;
  const progress = code ? remaining / period : 0;
  return (
    <Field label="One-time code" actions={<CopyButton label="Copy one-time code" onCopy={onCopy} />}>
      {failed ? (
        <span className="muted">Could not generate a code.</span>
      ) : (
        <span className="totp">
          <span className="mono totp-code">{code ? groupCode(code.code) : "••• •••"}</span>
          <svg className={`totp-ring${remaining <= 5 ? " totp-ring-low" : ""}`} viewBox="0 0 20 20" aria-hidden="true">
            <circle cx="10" cy="10" r="8" className="totp-ring-track" />
            <circle
              cx="10"
              cy="10"
              r="8"
              className="totp-ring-fill"
              strokeDasharray="50.27"
              strokeDashoffset={50.27 * (1 - progress)}
            />
          </svg>
          <span className="totp-seconds" aria-label={`${remaining} seconds remaining`}>
            {remaining}s
          </span>
        </span>
      )}
    </Field>
  );
}

/** One previous password, hidden until asked for. */
function PreviousPassword({ itemId, index, replacedAt }: { itemId: string; index: number; replacedAt: number }) {
  const toast = useToast();
  const secret = useRevealedSecret(useCallback(() => api.revealPreviousPassword(itemId, index), [itemId, index]));
  const toggle = () =>
    secret.value === null
      ? void secret.reveal().catch((e) => toast(e instanceof ApiError ? e.message : "Could not reveal.", "error"))
      : secret.hide();
  return (
    <li className="history-row">
      {secret.value === null ? (
        <span className="mono masked" aria-label="Hidden password">
          ••••••••••••
        </span>
      ) : (
        <span className="mono selectable revealed">{secret.value}</span>
      )}
      <span className="muted">Replaced {formatDate(replacedAt)}</span>
      <IconButton
        icon={secret.value === null ? "eye" : "eyeOff"}
        label={secret.value === null ? "Show previous password" : "Hide previous password"}
        onClick={toggle}
      />
    </li>
  );
}

/** Passwords this login used before. Loaded only when the user asks. */
function PasswordHistory({ itemId }: { itemId: string }) {
  const toast = useToast();
  const [dates, setDates] = useState<number[] | null>(null);
  const load = () =>
    api.passwordHistory(itemId).then(setDates, (e) =>
      toast(e instanceof ApiError ? e.message : "Could not load the history.", "error"),
    );
  if (dates === null) {
    return (
      <button className="row row-button" onClick={() => void load()}>
        <span className="row-label-inline">Password history</span>
        <Icon name="chevronDown" size={15} className="row-chevron" />
      </button>
    );
  }
  if (dates.length === 0) {
    return (
      <div className="row">
        <span className="row-label-inline">Password history</span>
        <span className="muted">No previous passwords</span>
      </div>
    );
  }
  return (
    <Field label="Previous passwords">
      <ul className="history-list">
        {dates.map((d, i) => (
          <PreviousPassword key={`${i}-${d}`} itemId={itemId} index={i} replacedAt={d} />
        ))}
      </ul>
    </Field>
  );
}

/** Passkeys saved on this login. Private keys never leave the core. */
function Passkeys({ itemId, readOnly }: { itemId: string; readOnly: boolean }) {
  const toast = useToast();
  const [list, setList] = useState<PasskeyInfo[] | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api.listPasskeys(itemId).then(
      (l) => !cancelled && setList(l),
      (e) => !cancelled && toast(e instanceof ApiError ? e.message : "Could not load passkeys.", "error"),
    );
    return () => {
      cancelled = true;
    };
  }, [itemId, toast]);

  const remove = async (credentialId: string) => {
    try {
      await api.deletePasskey(itemId, credentialId);
      setList((l) => l?.filter((p) => p.credentialId !== credentialId) ?? null);
      setConfirming(null);
      toast("Passkey deleted.");
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not delete the passkey.", "error");
    }
  };

  if (list === null || list.length === 0) return null;
  return (
    <Field label="Passkeys">
      <ul className="history-list">
        {list.map((p) => (
          <li className="history-row" key={p.credentialId}>
            <Icon name="key" size={15} />
            <span className="selectable">{p.rpId}</span>
            <span className="muted">
              {p.userName || p.displayName || "No account name"} · Saved {formatDate(p.createdAt)}
            </span>
            {confirming === p.credentialId ? (
              <span className="confirm">
                <span>You may lose access to {p.rpId}.</span>
                <button className="btn btn-small" onClick={() => setConfirming(null)}>
                  Keep
                </button>
                <button className="btn btn-small btn-danger" onClick={() => void remove(p.credentialId)}>
                  Delete
                </button>
              </span>
            ) : (
              <IconButton icon="trash" label="Delete passkey" onClick={() => setConfirming(p.credentialId)} disabled={readOnly} />
            )}
          </li>
        ))}
      </ul>
    </Field>
  );
}

export function ItemDetail({ item, readOnly, onEdit, onDelete }: Props) {
  const toast = useToast();
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
      .catch((e) => !cancelled && toast(e instanceof ApiError ? e.message : "Could not open the note.", "error"));
    return () => {
      cancelled = true;
      setContent(null);
    };
  }, [item.id, item.itemType, toast]);

  const onRevealError = (e: unknown) => toast(e instanceof ApiError ? e.message : "Could not reveal.", "error");

  const host = primaryHost(item);

  return (
    <article className="item">
      <header className="item-head" data-tauri-drag-region>
        <span className={`avatar avatar-lg avatar-${item.itemType}`} aria-hidden="true">
          {item.itemType === "secure_note" ? <Icon name="note" size={26} /> : monogram(item.title)}
        </span>
        <div className="item-head-text">
          <h2 className="item-title">{item.title}</h2>
          <p className="item-kind">{item.itemType === "login" ? (host ?? "Login") : "Secure note"}</p>
        </div>
        <div className="item-head-actions">
          <button className="btn btn-small" onClick={onEdit} disabled={readOnly}>
            <Icon name="edit" size={15} /> Edit
          </button>
        </div>
      </header>

      {item.itemType === "login" && (
        <>
          <div className="group">
            {item.username && (
              <Field
                label="Username"
                actions={<CopyButton label="Copy username" onCopy={() => copy("username", "Username")} />}
              >
                <span className="selectable">{item.username}</span>
              </Field>
            )}

            {item.hasPassword && (
              <Field
                label="Password"
                actions={
                  <>
                    <IconButton
                      icon={password.value === null ? "eye" : "eyeOff"}
                      label={password.value === null ? "Show password" : "Hide password"}
                      onClick={() =>
                        password.value === null ? void password.reveal().catch(onRevealError) : password.hide()
                      }
                    />
                    <CopyButton label="Copy password" onCopy={() => copy("password", "Password")} />
                  </>
                }
              >
                <span className="secret" data-revealed={password.value !== null}>
                  {password.value === null ? (
                    <span className="mono masked" aria-label="Hidden password">
                      ••••••••••••
                    </span>
                  ) : (
                    <span className="mono selectable revealed">{password.value}</span>
                  )}
                </span>
              </Field>
            )}

            {item.hasTotp && <TotpField item={item} onCopy={() => copy("totp", "One-time code")} />}
          </div>

          {item.hasPasskey && (
            <div className="group">
              <Passkeys itemId={item.id} readOnly={readOnly} />
            </div>
          )}

          {item.urls.length > 0 && (
            <div className="group">
              {item.urls.map((u) => (
                <div className="row" key={u.url}>
                  <div className="row-main">
                    <div className="row-label">Website</div>
                    <div className="row-value url-value">
                      <span className="selectable">{u.url}</span>
                    </div>
                  </div>
                  <span className="url-match">
                    {u.matchType === "domain" ? "Whole site" : u.matchType === "origin" ? "Exact site" : "Exact page"}
                  </span>
                </div>
              ))}
            </div>
          )}

          {item.hasNotes && (
            <div className="group">
              <Field
                label="Notes"
                actions={
                  <IconButton
                    icon={notes.value === null ? "eye" : "eyeOff"}
                    label={notes.value === null ? "Show notes" : "Hide notes"}
                    onClick={() => (notes.value === null ? void notes.reveal().catch(onRevealError) : notes.hide())}
                  />
                }
              >
                {notes.value === null ? (
                  <span className="muted">Hidden</span>
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
            <p className="muted">Decrypting…</p>
          ) : (
            <p className="note-body selectable">{content}</p>
          )}
        </div>
      )}

      <footer className="item-foot">
        <p className="muted">
          Created {formatDate(item.createdAt)} · Changed {formatDate(item.updatedAt)}
        </p>
        {confirmDelete ? (
          <div className="confirm">
            <span>
              Delete “{item.title}” permanently?
              {item.hasPasskey && " Its passkeys go with it, and you may lose access to those sites."}
            </span>
            <button className="btn btn-small" onClick={() => setConfirmDelete(false)}>
              Keep
            </button>
            <button className="btn btn-small btn-danger" onClick={onDelete}>
              Delete
            </button>
          </div>
        ) : (
          <button
            className="btn btn-small btn-quiet-danger"
            onClick={() => setConfirmDelete(true)}
            disabled={readOnly}
          >
            <Icon name="trash" size={15} /> Delete
          </button>
        )}
      </footer>
    </article>
  );
}

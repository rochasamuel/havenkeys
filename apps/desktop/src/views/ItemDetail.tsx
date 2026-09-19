import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, ApiError } from "../lib/api";
import type { CopyField, ItemOverview } from "../lib/types";
import { formatDate, groupCode, monogram } from "../lib/format";
import { useRevealedSecret, useTotp } from "../lib/hooks";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";

interface Props {
  item: ItemOverview;
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
      } catch (e) {
        toast(e instanceof ApiError ? e.message : "Could not copy.", "error");
      }
    },
    [itemId, toast],
  );
}

function Field({ label, children, actions }: { label: string; children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="field">
      <div className="field-label">{label}</div>
      <div className="field-body">
        <div className="field-value">{children}</div>
        {actions && <div className="field-actions">{actions}</div>}
      </div>
    </div>
  );
}

function IconButton({ icon, label, onClick }: { icon: Parameters<typeof Icon>[0]["name"]; label: string; onClick: () => void }) {
  return (
    <button className="icon-btn" onClick={onClick} title={label} aria-label={label}>
      <Icon name={icon} size={16} />
    </button>
  );
}

function TotpField({ item, onCopy }: { item: ItemOverview; onCopy: () => void }) {
  const { code, remaining, failed } = useTotp(item.id, true);
  const period = code?.period ?? 30;
  const progress = code ? remaining / period : 0;
  return (
    <Field label="One-time code" actions={<IconButton icon="copy" label="Copy one-time code" onClick={onCopy} />}>
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
              strokeDasharray={`${progress * 50.27} 50.27`}
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

export function ItemDetail({ item, onEdit, onDelete }: Props) {
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

  return (
    <article className="item">
      <header className="item-head">
        <span className={`avatar avatar-lg avatar-${item.itemType}`} aria-hidden="true">
          {item.itemType === "secure_note" ? <Icon name="note" size={24} /> : monogram(item.title)}
        </span>
        <div className="item-head-text">
          <h2 className="item-title">{item.title}</h2>
          <p className="item-kind">{item.itemType === "login" ? "Login" : "Secure note"}</p>
        </div>
        <div className="item-head-actions">
          <button className="btn btn-small" onClick={onEdit}>
            <Icon name="edit" size={15} /> Edit
          </button>
        </div>
      </header>

      {item.itemType === "login" && (
        <div className="fields">
          {item.username && (
            <Field
              label="Username"
              actions={<IconButton icon="copy" label="Copy username" onClick={() => void copy("username", "Username")} />}
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
                    onClick={() => (password.value === null ? void password.reveal().catch(onRevealError) : password.hide())}
                  />
                  <IconButton icon="copy" label="Copy password" onClick={() => void copy("password", "Password")} />
                </>
              }
            >
              {password.value === null ? (
                <span className="mono masked" aria-label="Hidden password">
                  ••••••••••••
                </span>
              ) : (
                <span className="mono selectable revealed">{password.value}</span>
              )}
            </Field>
          )}

          {item.hasTotp && <TotpField item={item} onCopy={() => void copy("totp", "One-time code")} />}

          {item.urls.length > 0 && (
            <Field label={item.urls.length > 1 ? "Websites" : "Website"}>
              <ul className="url-list">
                {item.urls.map((u) => (
                  <li key={u.url}>
                    <Icon name="globe" size={14} />
                    <span className="selectable">{u.url}</span>
                    <span className="muted url-match">
                      {u.matchType === "domain" ? "matches subdomains" : u.matchType === "origin" ? "exact site" : "exact page"}
                    </span>
                  </li>
                ))}
              </ul>
            </Field>
          )}

          {item.hasNotes && (
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
          )}
        </div>
      )}

      {item.itemType === "secure_note" && (
        <div className="note-sheet">
          {content === null ? <p className="muted">Decrypting…</p> : <p className="note-body selectable">{content}</p>}
        </div>
      )}

      <footer className="item-foot">
        <p className="muted">
          Created {formatDate(item.createdAt)}. Last changed {formatDate(item.updatedAt)}.
        </p>
        {confirmDelete ? (
          <div className="confirm">
            <span>Delete “{item.title}” permanently?</span>
            <button className="btn btn-small btn-danger" onClick={onDelete}>
              Delete
            </button>
            <button className="btn btn-small" onClick={() => setConfirmDelete(false)}>
              Keep
            </button>
          </div>
        ) : (
          <button className="btn btn-small btn-quiet-danger" onClick={() => setConfirmDelete(true)}>
            <Icon name="trash" size={15} /> Delete
          </button>
        )}
      </footer>
    </article>
  );
}

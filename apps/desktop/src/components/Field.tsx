import type { ReactNode } from "react";
import { groupCode } from "../lib/format";
import { useTotp } from "../lib/hooks";
import type { TotpCode } from "../lib/types";
import { useI18n } from "../i18n/context";
import { Icon } from "./Icon";

/** A detail row: its label above the value, actions at the end. */
export function Field({ label, children, actions }: { label: string; children: ReactNode; actions?: ReactNode }) {
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

export function IconButton({
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

/**
 * A live one-time code with its countdown ring. The login's own TOTP and
 * every OTP custom field use this row; the code comes from Rust, never the
 * secret. `loadKey` restarts it.
 */
export function TotpField({
  label,
  load,
  loadKey,
  actions,
}: {
  label: string;
  load: () => Promise<TotpCode>;
  loadKey: string;
  actions?: ReactNode;
}) {
  const { t } = useI18n();
  const { code, remaining, failed } = useTotp(load, loadKey, true);
  const period = code?.period ?? 30;
  const progress = code ? remaining / period : 0;
  return (
    <Field label={label} actions={actions}>
      {failed ? (
        <span className="muted">{t.detail.codeFailed}</span>
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
          <span className="totp-seconds" aria-label={t.detail.secondsRemaining(remaining)}>
            {remaining}s
          </span>
        </span>
      )}
    </Field>
  );
}

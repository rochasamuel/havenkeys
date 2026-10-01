import { useState } from "react";
import { Icon } from "./Icon";
import { api } from "../lib/api";
import { canScan, EMPTY, KEEP, scanLabel, type SecretEdit } from "../lib/secretEdit";
import type { ScannedTotp } from "../lib/types";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

/**
 * A one-time-code setup: kept, removed, scanned or typed. The login's own
 * TOTP and every OTP custom field use this row. A scan keeps the secret in
 * Rust; the editor only holds its token.
 *
 * `inline` drops the row wrapper, for a field that already sits in a row.
 */
export function TotpEdit({
  edit,
  onChange,
  disabled,
  ariaLabel,
  onError,
  inline = false,
}: {
  edit: SecretEdit;
  onChange: (edit: SecretEdit) => void;
  disabled: boolean;
  ariaLabel: string;
  onError: (message: string) => void;
  inline?: boolean;
}) {
  const { t } = useI18n();
  const [scanning, setScanning] = useState(false);
  const [choices, setChoices] = useState<ScannedTotp[] | null>(null);

  async function scan() {
    setScanning(true);
    setChoices(null);
    try {
      const found = await api.scanTotpQr();
      if (found.length === 1) pick(found[0]!);
      else setChoices(found);
    } catch (e) {
      onError(errorMessage(e, t, t.editor.scanFailed));
    } finally {
      setScanning(false);
    }
  }

  function pick(code: ScannedTotp) {
    setChoices(null);
    onChange({ mode: "scanned", token: code.token, label: scanLabel(code, t.editor.scanUnnamed) });
  }

  return (
    <>
      <div className={inline ? "cf-otp" : "row edit-row"}>
        {edit.mode === "keep" ? (
          <div className="edit-secret">
            <span className="muted">{t.editor.setUp}</span>
            <span className="edit-secret-actions">
              <button type="button" className="btn btn-small" onClick={() => onChange(EMPTY)} disabled={disabled}>
                {t.editor.replace}
              </button>
              <button
                type="button"
                className="btn btn-small btn-quiet-danger"
                onClick={() => onChange({ mode: "clear" })}
                disabled={disabled}
              >
                {t.common.remove}
              </button>
            </span>
          </div>
        ) : edit.mode === "clear" ? (
          <div className="edit-secret">
            <span className="muted">{t.editor.codesRemoved}</span>
            <span className="edit-secret-actions">
              <button type="button" className="btn btn-small" onClick={() => onChange(KEEP)} disabled={disabled}>
                {t.common.undo}
              </button>
            </span>
          </div>
        ) : edit.mode === "scanned" ? (
          <div className="edit-secret">
            <span className="scanned-code">
              <Icon name="check" size={15} /> {t.editor.scanned(edit.label)}
            </span>
            <span className="edit-secret-actions">
              <button type="button" className="btn btn-small" onClick={() => onChange(EMPTY)} disabled={disabled}>
                {t.common.undo}
              </button>
            </span>
          </div>
        ) : (
          <div className="edit-secret">
            <input
              className="edit-input mono"
              type="password"
              value={edit.value}
              onChange={(e) => {
                onChange({ mode: "set", value: e.target.value });
                setChoices(null);
              }}
              placeholder={t.editor.totpPlaceholder}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              aria-label={ariaLabel}
              disabled={disabled}
            />
            {canScan(edit) && (
              <span className="edit-secret-actions">
                <button
                  type="button"
                  className="icon-btn"
                  onClick={() => void scan()}
                  disabled={scanning || disabled}
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
      {choices && choices.length > 1 && (
        <div className={inline ? "cf-scan-choices scan-choices" : "row scan-choices"}>
          <span className="muted">{t.editor.scanPick}</span>
          {choices.map((code) => (
            <button key={code.token} type="button" className="btn btn-small" onClick={() => pick(code)}>
              {scanLabel(code, t.editor.scanUnnamed)}
            </button>
          ))}
        </div>
      )}
    </>
  );
}

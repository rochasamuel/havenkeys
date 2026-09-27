import type { ScannedTotp, SecretUpdate } from "./types";

/**
 * How the editor holds a secret field. `scanned` is TOTP only: the secret
 * stays in Rust and the editor keeps the token and a label to show.
 */
export type SecretEdit =
  | { mode: "keep" }
  | { mode: "clear" }
  | { mode: "set"; value: string }
  | { mode: "scanned"; token: string; label: string };

export const KEEP: SecretEdit = { mode: "keep" };
export const EMPTY: SecretEdit = { mode: "set", value: "" };

export function toUpdate(edit: SecretEdit): SecretUpdate {
  if (edit.mode === "set") return edit.value ? { op: "set", value: edit.value } : { op: "clear" };
  if (edit.mode === "scanned") return { op: "scanned", value: edit.token };
  return { op: edit.mode };
}

/** "Issuer · account", or `unnamed` when the QR code carried neither. */
export function scanLabel(code: ScannedTotp, unnamed: string): string {
  const parts = [code.issuer, code.account]
    .map((s) => s?.trim())
    .filter((s): s is string => !!s);
  return parts.length > 0 ? parts.join(" · ") : unnamed;
}

/** The QR button belongs in the row while the field is empty. */
export function canScan(edit: SecretEdit): boolean {
  return edit.mode === "set" && edit.value === "";
}

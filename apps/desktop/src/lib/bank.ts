// Display helpers for the Bank Account item (spec 2026-11-03-bank-account).
// Rust validates and stores; this only decides what to show.

import type { BankSummary } from "./types";

/** The row's two labels; the caller passes the translated ones. */
export interface BankLabels {
  ag: string;
  account: string;
}

const DEFAULT_LABELS: BankLabels = { ag: "Branch", account: "Account" };

/** A number and its check digit as people write them: `12345-6`. */
export const joinDigit = (number: string, digit: string | null | undefined) => (digit ? `${number}-${digit}` : number);

/** A pasted `12345-6` into its number and check digit; spaces and dots go, `x` becomes `X`. */
export function splitPasted(text: string): { number: string; digit: string | null } {
  const compact = text.replace(/[\s.]/g, "").toUpperCase();
  const dash = compact.lastIndexOf("-");
  if (dash < 0) return { number: compact, digit: null };
  return { number: compact.slice(0, dash), digit: compact.slice(dash + 1) || null };
}

/** A bank account's row line: `Branch 1234 · Account •••45-6`; the bank's name when it has neither. */
export function bankSubtitle(b: BankSummary, labels: BankLabels = DEFAULT_LABELS): string {
  const parts = [b.branch && `${labels.ag} ${b.branch}`, b.accountLast3 && `${labels.account} •••${b.accountLast3}`].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : (b.bank ?? "");
}

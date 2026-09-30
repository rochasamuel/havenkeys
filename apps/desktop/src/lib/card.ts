// Display helpers for the Card item (spec 2026-09-29-card-item). Rust
// validates and stores; this only decides what to show.

import type { CardBrand, CardExpiry, CardSummary } from "./types";

/** The networks' own names; `other` is translated (t.card.otherBrand). */
export const BRAND_NAMES: Record<Exclude<CardBrand, "other">, string> = {
  visa: "Visa",
  mastercard: "Mastercard",
  amex: "American Express",
  elo: "Elo",
  hipercard: "Hipercard",
  diners: "Diners Club",
  discover: "Discover",
  jcb: "JCB",
  unionpay: "UnionPay",
  maestro: "Maestro",
};

/** The editor's Type choices, in this order, then Other. */
export const CARD_BRAND_CHOICES = Object.keys(BRAND_NAMES) as Array<Exclude<CardBrand, "other">>;

export const digitsOnly = (s: string) => s.replace(/[\s-]/g, "");

/** Groups as printed on the card: Amex 4-6-5, 14-digit Diners 4-6-4, the rest in fours. */
export function groupNumber(digits: string, brand: CardBrand | null): string {
  const cut = (sizes: number[]) => {
    const out: string[] = [];
    let at = 0;
    for (const n of sizes) {
      if (at >= digits.length) break;
      out.push(digits.slice(at, at + n));
      at += n;
    }
    if (at < digits.length) out.push(digits.slice(at));
    return out.join(" ");
  };
  if (brand === "amex") return cut([4, 6, 5]);
  if (brand === "diners" && digits.length === 14) return cut([4, 6, 4]);
  return digits.replace(/(\d{4})(?=\d)/g, "$1 ");
}

export const maskedNumber = (last4: string | null | undefined) => (last4 ? `•••• •••• •••• ${last4}` : "••••");

export function formatExpiry(e: CardExpiry): string {
  const [year, month] = e.split("-");
  return `${month}/${year}`;
}

/** `MM/YYYY` or `MM/YY` (→ 20YY); empty is "no expiry". */
export function parseExpiryInput(s: string): { ok: true; value: CardExpiry | null } | { ok: false } {
  const text = s.trim();
  if (text === "") return { ok: true, value: null };
  const m = /^(\d{1,2})\s*\/\s*(\d{2}|\d{4})$/.exec(text);
  if (!m) return { ok: false };
  const month = Number(m[1]);
  const year = m[2]!.length === 2 ? 2000 + Number(m[2]) : Number(m[2]);
  if (month < 1 || month > 12 || year < 1970 || year > 2099) return { ok: false };
  return { ok: true, value: `${year}-${String(month).padStart(2, "0")}` };
}

/** True once the expiry month has passed. */
export function isExpired(e: CardExpiry, now: Date): boolean {
  const [year, month] = e.split("-").map(Number) as [number, number];
  const y = now.getFullYear();
  return year < y || (year === y && month < now.getMonth() + 1);
}

/** The list row's second line: `•••• 7609 · 11/2033`. */
export function cardSubtitle(card: CardSummary | undefined): string | null {
  if (!card) return null;
  const parts = [card.last4 ? `•••• ${card.last4}` : null, card.expiry ? formatExpiry(card.expiry) : null].filter(
    (p): p is string => p !== null,
  );
  return parts.length ? parts.join(" · ") : null;
}

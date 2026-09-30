// Card rows for the menu and popup: display expiry and expired-last order.
// Overview data only (find_cards); nothing here sees a number.

import type { CardMatch } from "@havenkeys/protocol";
import type { CardRowView } from "../messaging/inline";

/** "2033-04" → "04/33". */
export function displayExpiry(iso: string | null): string | null {
  const m = iso ? /^(\d{4})-(\d{2})$/.exec(iso) : null;
  return m ? `${m[2]}/${(m[1] as string).slice(2)}` : null;
}

/** The card's last valid month has passed (local time). */
export function isExpired(iso: string | null, now: number): boolean {
  if (!iso) return false;
  const d = new Date(now);
  return iso < `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

export function cardRows(cards: readonly CardMatch[], now: number): CardRowView[] {
  const rows = cards.map((c) => ({
    id: c.id,
    title: c.title,
    brand: c.brand,
    last4: c.last4,
    expiry: displayExpiry(c.expiry),
    expired: isExpired(c.expiry, now),
  }));
  return [...rows.filter((r) => !r.expired), ...rows.filter((r) => r.expired)];
}

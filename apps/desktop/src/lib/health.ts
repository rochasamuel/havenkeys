// Vault health report helpers. The report holds IDs and check kinds only;
// titles and usernames come from the item overviews the vault already shows.

import type { HealthCheck, HealthCounts, HealthReport } from "./types";

/** Watchtower's order, then "old". */
export const CHECK_ORDER: HealthCheck[] = ["reused", "weak", "insecure", "duplicate", "passkey", "two_factor", "old"];

const COUNT_KEY: Record<HealthCheck, keyof HealthCounts> = {
  weak: "weak",
  reused: "reused",
  old: "old",
  passkey: "passkey",
  two_factor: "twoFactor",
  insecure: "insecure",
  duplicate: "duplicate",
};

export function countOf(r: HealthReport, check: HealthCheck): number {
  return r.counts[COUNT_KEY[check]];
}

export function totalIssues(r: HealthReport): number {
  return CHECK_ORDER.reduce((sum, c) => sum + countOf(r, c), 0);
}

export function checksFor(r: HealthReport | null, id: string): HealthCheck[] {
  return r?.issues.find((i) => i.itemId === id)?.checks ?? [];
}

export function groupSize(r: HealthReport, kind: "reused" | "duplicate", group: number): number {
  return r.issues.filter((i) => (kind === "reused" ? i.reusedGroup : i.duplicateGroup) === group).length;
}

// The core numbers groups before dismissals, so a login whose group-mates
// dismissed the check can be alone in its visible group. A reused password
// is always in at least 2 logins, and a duplicate has at least 1 other.

/** How many logins share this login's password, for "Used in N logins". */
export function reusedCount(r: HealthReport, id: string): number {
  const group = r.issues.find((i) => i.itemId === id)?.reusedGroup;
  return Math.max(2, group != null ? groupSize(r, "reused", group) : 0);
}

/** How many other logins duplicate this one, for "Duplicate of N other logins". */
export function duplicateOthers(r: HealthReport, id: string): number {
  const group = r.issues.find((i) => i.itemId === id)?.duplicateGroup;
  return Math.max(1, group != null ? groupSize(r, "duplicate", group) - 1 : 0);
}

export type HealthFilter = HealthCheck | "dismissed" | "all";

export interface HealthRow {
  itemId: string;
  checks: HealthCheck[];
  reusedGroup: number | null;
  duplicateGroup: number | null;
  /** The site publishes a passkey or two-factor setup guide. */
  help: boolean;
  dismissed: boolean;
}

export function rowsFor(r: HealthReport, filter: HealthFilter): HealthRow[] {
  if (filter === "dismissed") {
    return r.dismissed.map((d) => ({ itemId: d.itemId, checks: d.checks, reusedGroup: null, duplicateGroup: null, help: false, dismissed: true }));
  }
  return r.issues
    .filter((i) => filter === "all" || i.checks.includes(filter))
    .map((i) => ({ ...i, dismissed: false }));
}

/** The checks already dismissed on a login (the command replaces the whole list). */
export function dismissedFor(r: HealthReport, id: string): HealthCheck[] {
  return r.dismissed.find((d) => d.itemId === id)?.checks ?? [];
}

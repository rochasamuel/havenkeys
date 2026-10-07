import type { ReactNode } from "react";
import type { HealthCheck, HealthReport } from "../lib/types";
import { duplicateOthers, reusedCount } from "../lib/health";
import { useI18n } from "../i18n/context";
import type { Messages } from "../i18n/en";

export function chipLabel(t: Messages, check: HealthCheck, report: HealthReport, itemId: string): string {
  switch (check) {
    case "reused":
      return t.health.chip.reused(reusedCount(report, itemId));
    case "duplicate":
      return t.health.chip.duplicate(duplicateOthers(report, itemId));
    default:
      return t.health.chip[check];
  }
}

interface Props {
  checks: HealthCheck[];
  report: HealthReport;
  itemId: string;
  /** A control inside each chip (the health view's dismiss and undo). */
  control?: (check: HealthCheck, label: string) => ReactNode;
}

export function HealthChips({ checks, report, itemId, control }: Props) {
  const { t } = useI18n();
  if (checks.length === 0) return null;
  return (
    <ul className="health-chips" aria-label={t.health.title}>
      {checks.map((c) => {
        const label = chipLabel(t, c, report, itemId);
        return (
          <li key={c} className={`chip chip-health chip-health-${c}`}>
            <span className="chip-text">{label}</span>
            {control?.(c, label)}
          </li>
        );
      })}
    </ul>
  );
}

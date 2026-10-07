import { useState } from "react";
import { api } from "../lib/api";
import type { HealthCheck, HealthReport, ItemOverview } from "../lib/types";
import { CHECK_ORDER, countOf, dismissedFor, rowsFor, totalIssues, type HealthFilter, type HealthRow } from "../lib/health";
import { monogram } from "../lib/format";
import { HealthChips } from "../components/HealthChips";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

interface Props {
  items: ItemOverview[];
  report: HealthReport | null;
  loading: boolean;
  /** Offline: dismissing would fail, so those controls are disabled up front. */
  readOnly: boolean;
  onOpen: (id: string) => void;
  onEdit: (id: string) => void;
  /** A dismissal was saved: the caller reloads the report. */
  onChanged: () => void;
}

/** Checks a new password fixes. */
const PASSWORD_CHECKS: HealthCheck[] = ["weak", "reused", "old"];
/** Checks the site's own help page explains. */
const HELP_CHECKS: HealthCheck[] = ["passkey", "two_factor"];

export function HealthView({ items, report, loading, readOnly, onOpen, onEdit, onChanged }: Props) {
  const toast = useToast();
  const { t } = useI18n();
  const [filter, setFilter] = useState<HealthFilter>("all");
  const [busy, setBusy] = useState<string | null>(null);

  async function saveIgnored(id: string, checks: HealthCheck[]) {
    setBusy(id);
    try {
      await api.setHealthIgnored(id, checks);
      onChanged();
    } catch (e) {
      toast(errorMessage(e, t, t.health.dismissFailed), "error");
    } finally {
      setBusy(null);
    }
  }

  function dismiss(r: HealthReport, id: string, check: HealthCheck) {
    const already = dismissedFor(r, id);
    void saveIgnored(id, already.includes(check) ? already : [...already, check]);
  }

  function undo(r: HealthReport, id: string, check: HealthCheck) {
    void saveIgnored(id, dismissedFor(r, id).filter((c) => c !== check));
  }

  async function openHelp(id: string, check: HealthCheck) {
    try {
      await api.openHealthHelp(id, check);
    } catch (e) {
      toast(errorMessage(e, t), "error");
    }
  }

  function renderRow(r: HealthReport, row: HealthRow) {
    const item = items.find((i) => i.id === row.itemId);
    if (!item) return null;
    const single = filter !== "all" && filter !== "dismissed" ? filter : null;
    const passwordFix = single
      ? PASSWORD_CHECKS.includes(single)
      : !row.dismissed && row.checks.some((c) => PASSWORD_CHECKS.includes(c));
    const help = single
      ? HELP_CHECKS.includes(single)
        ? single
        : null
      : row.dismissed
        ? null
        : (row.checks.find((c) => HELP_CHECKS.includes(c)) ?? null);
    const disabled = readOnly || busy === row.itemId;

    // Under "All issues" and "Dismissed" each chip carries its own control;
    // under one check's filter the row's Dismiss button acts on that check.
    const chipControl =
      single === null
        ? (check: HealthCheck, label: string) =>
            row.dismissed ? (
              <button
                type="button"
                className="chip-action"
                aria-label={`${t.health.undo}: ${label}`}
                disabled={disabled}
                onClick={() => undo(r, row.itemId, check)}
              >
                {t.health.undo}
              </button>
            ) : (
              <button
                type="button"
                className="chip-action chip-action-icon"
                aria-label={`${t.health.dismiss}: ${label}`}
                title={t.health.dismiss}
                disabled={disabled}
                onClick={() => dismiss(r, row.itemId, check)}
              >
                <Icon name="x" size={12} />
                <span className="sr-only">{t.health.dismiss}</span>
              </button>
            )
        : undefined;

    return (
      <li key={row.itemId} className="health-row">
        <span className={`avatar avatar-${item.itemType}`} aria-hidden="true">
          {monogram(item.title)}
        </span>
        <div className="health-row-main">
          {/* User data may be cut with an ellipsis (data-truncate); our own words never are. */}
          <p className="health-row-title" data-truncate="">
            {item.title}
          </p>
          {item.username && (
            <p className="health-row-sub" data-truncate="">
              {item.username}
            </p>
          )}
          <HealthChips checks={single ? [single] : row.checks} report={r} itemId={row.itemId} control={chipControl} />
        </div>
        <div className="health-row-actions">
          {passwordFix && (
            <button type="button" className="btn btn-small" disabled={readOnly} onClick={() => onEdit(row.itemId)}>
              {t.health.changePassword}
            </button>
          )}
          {help && (
            <button type="button" className="btn btn-small" onClick={() => void openHelp(row.itemId, help)}>
              {t.health.howToEnable}
            </button>
          )}
          {single && (
            <button
              type="button"
              className="btn btn-small btn-quiet"
              disabled={disabled}
              onClick={() => dismiss(r, row.itemId, single)}
            >
              {t.health.dismiss}
            </button>
          )}
          <button type="button" className="btn btn-small btn-quiet" onClick={() => onOpen(row.itemId)}>
            {t.health.open}
          </button>
        </div>
      </li>
    );
  }

  function body() {
    if (!report) {
      return loading ? (
        <p className="health-status muted" role="status">
          {t.health.loading}
        </p>
      ) : (
        <p className="health-status muted" role="status">
          {t.health.loadFailed}
        </p>
      );
    }
    const rows = rowsFor(report, filter)
      .map((row) => renderRow(report, row))
      .filter((r) => r !== null);
    const heading =
      filter === "all" ? t.health.all : filter === "dismissed" ? t.health.dismissedFilter : t.health.cards[filter].title;
    return (
      <>
        <ul className="group health-cards">
          {CHECK_ORDER.map((check) => {
            const n = countOf(report, check);
            return (
              <li key={check}>
                <button
                  type="button"
                  className={`health-card${n === 0 ? " is-clear" : ""}`}
                  aria-pressed={filter === check}
                  onClick={() => setFilter(filter === check ? "all" : check)}
                >
                  <span className="health-card-title">{t.health.cards[check].title}</span>
                  <span className="health-card-count">{n}</span>
                  <span className="sr-only">{t.health.showItems}</span>
                  <Icon name="chevronRight" size={15} className="health-card-chevron" />
                </button>
              </li>
            );
          })}
        </ul>

        <div className="health-list-head">
          <h3 className="group-title">{heading}</h3>
          <div className="segmented" role="group" aria-label={t.health.title}>
            <button type="button" aria-pressed={filter === "all"} onClick={() => setFilter("all")}>
              {t.health.all}
            </button>
            <button type="button" aria-pressed={filter === "dismissed"} onClick={() => setFilter("dismissed")}>
              {t.health.dismissedFilter}
            </button>
          </div>
        </div>
        {filter !== "all" && filter !== "dismissed" && <p className="health-explain muted">{t.health.cards[filter].body}</p>}

        {rows.length > 0 ? (
          <ul className="group health-list">{rows}</ul>
        ) : (
          <p className="health-status muted">
            {totalIssues(report) === 0 && filter === "all" ? t.health.empty : t.health.emptyFiltered}
          </p>
        )}
      </>
    );
  }

  return (
    <section className="tool health" aria-labelledby="health-title">
      <header className="tool-head" data-tauri-drag-region>
        <h2 id="health-title">{t.health.title}</h2>
        <p className="tool-lede">{t.health.intro}</p>
      </header>
      {body()}
    </section>
  );
}

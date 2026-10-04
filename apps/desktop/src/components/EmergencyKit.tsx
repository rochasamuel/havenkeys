import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import type { EmergencyKit as Kit } from "../lib/types";
import { formatDate } from "../lib/format";
import { Icon } from "./Icon";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

/** The QR code as one SVG path (one square per dark module). */
export function QrCode({ size, modules, label, className }: { size: number; modules: boolean[]; label?: string; className?: string }) {
  const { t } = useI18n();
  const d = useMemo(() => {
    const parts: string[] = [];
    modules.forEach((dark, i) => {
      if (dark) parts.push(`M${(i % size) + 4} ${Math.floor(i / size) + 4}h1v1h-1z`);
    });
    return parts.join("");
  }, [size, modules]);
  const box = size + 8; // 4-module quiet zone on each side
  return (
    <svg className={className ?? "kit-qr"} viewBox={`0 0 ${box} ${box}`} role="img" aria-label={label ?? t.kit.qrLabel} shapeRendering="crispEdges">
      <rect width={box} height={box} fill="#fff" />
      <path d={d} fill="#000" />
    </svg>
  );
}

interface Props {
  /** First time after creating a vault or adding a Secret Key: require confirmation. */
  onDone?: () => void;
}

/**
 * The Emergency Kit: the Secret Key the user needs, with the master
 * password, to open the vault on another device. Fetched on demand and
 * dropped from memory when this component unmounts (for example on lock).
 */
export function EmergencyKit({ onDone }: Props) {
  const { t, dateLocale } = useI18n();
  const [kit, setKit] = useState<Kit | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api.emergencyKit().then(
      (k) => !cancelled && setKit(k),
      (e) => !cancelled && setError(errorMessage(e, t, t.kit.loadFailed)),
    );
    return () => {
      cancelled = true;
      setKit(null);
    };
    // `t` is left out: a language change must not fetch the Secret Key again.
  }, []);

  if (error) return <p className="form-error">{error}</p>;
  if (!kit) return <p className="muted">{t.kit.preparing}</p>;

  return (
    <div className="kit">
      <article className="kit-sheet" aria-label={t.kit.sheetLabel}>
        <header className="kit-head">
          <h2>{t.kit.heading}</h2>
          <p>{t.kit.created(formatDate(kit.createdAt, dateLocale))}</p>
        </header>
        <p className="kit-lede">
          {t.kit.ledeBefore}
          <strong>{t.kit.ledeBoth}</strong>
          {t.kit.ledeAfter}
        </p>
        <div className="kit-body">
          <div className="kit-fields">
            <div className="kit-field">
              <span>{t.common.secretKey}</span>
              <code className="kit-key">{kit.secretKey}</code>
            </div>
            <div className="kit-field">
              <span>{t.common.masterPassword}</span>
              <div className="kit-blank" aria-label={t.kit.blankLine} />
            </div>
            <div className="kit-field kit-small">
              <span>{t.common.email}</span>
              <code>{kit.email}</code>
            </div>
            <div className="kit-field kit-small">
              <span>{t.common.server}</span>
              <code>{kit.serverUrl}</code>
            </div>
            <div className="kit-field kit-small">
              <span>{t.kit.accountId}</span>
              <code>{kit.accountId}</code>
            </div>
          </div>
          <figure className="kit-figure">
            <QrCode size={kit.qrSize} modules={kit.qrModules} />
            <figcaption>{t.kit.qrCaption}</figcaption>
          </figure>
        </div>
        <p className="kit-foot">{t.kit.foot}</p>
      </article>

      <div className="kit-actions">
        <button className="btn" type="button" onClick={() => window.print()}>
          <Icon name="printer" size={16} /> {t.kit.print}
        </button>
        {onDone && (
          <>
            <label className="check">
              <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
              <span>{t.kit.savedCheck}</span>
            </label>
            <button className="btn btn-brass" type="button" disabled={!saved} onClick={onDone}>
              {t.common.continue}
            </button>
          </>
        )}
      </div>
    </div>
  );
}

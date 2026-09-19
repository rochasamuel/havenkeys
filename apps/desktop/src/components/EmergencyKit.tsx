import { useEffect, useMemo, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { EmergencyKit as Kit } from "../lib/types";
import { formatDate } from "../lib/format";

/** The QR code as one SVG path (one square per dark module). */
function QrCode({ size, modules }: { size: number; modules: boolean[] }) {
  const d = useMemo(() => {
    const parts: string[] = [];
    modules.forEach((dark, i) => {
      if (dark) parts.push(`M${(i % size) + 4} ${Math.floor(i / size) + 4}h1v1h-1z`);
    });
    return parts.join("");
  }, [size, modules]);
  const box = size + 8; // 4-module quiet zone on each side
  return (
    <svg className="kit-qr" viewBox={`0 0 ${box} ${box}`} role="img" aria-label="Secret Key QR code" shapeRendering="crispEdges">
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
  const [kit, setKit] = useState<Kit | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api.emergencyKit().then(
      (k) => !cancelled && setKit(k),
      (e) => !cancelled && setError(e instanceof ApiError ? e.message : "Could not load the Emergency Kit."),
    );
    return () => {
      cancelled = true;
      setKit(null);
    };
  }, []);

  if (error) return <p className="form-error">{error}</p>;
  if (!kit) return <p className="muted">Preparing your Emergency Kit…</p>;

  return (
    <div className="kit">
      <article className="kit-sheet" aria-label="Emergency Kit">
        <header className="kit-head">
          <h2>HavenKeys Emergency Kit</h2>
          <p>Created {formatDate(kit.createdAt)}</p>
        </header>
        <p className="kit-lede">
          To open your vault on a new computer or phone you need <strong>both</strong> your master password and this Secret
          Key. Print this page or save it somewhere safe and offline. Anyone who has both can open your vault.
        </p>
        <div className="kit-body">
          <div className="kit-fields">
            <div className="kit-field">
              <span>Secret Key</span>
              <code className="kit-key">{kit.secretKey}</code>
            </div>
            <div className="kit-field">
              <span>Master password</span>
              <div className="kit-blank" aria-label="Blank line to write your master password, if you choose" />
            </div>
            <div className="kit-field">
              <span>Sync folder</span>
              <div className="kit-blank" />
            </div>
            <div className="kit-field kit-small">
              <span>Vault ID</span>
              <code>{kit.vaultId}</code>
            </div>
          </div>
          <figure className="kit-figure">
            <QrCode size={kit.qrSize} modules={kit.qrModules} />
            <figcaption>Scan with the HavenKeys mobile app to enter your Secret Key.</figcaption>
          </figure>
        </div>
        <p className="kit-foot">
          HavenKeys cannot recover your master password or Secret Key. If you lose this kit, you can view it again on any
          device where your vault is unlocked (Settings → Secret Key &amp; sync).
        </p>
      </article>

      <div className="kit-actions">
        <button className="btn btn-primary" type="button" onClick={() => window.print()}>
          Print or save as PDF
        </button>
        {onDone && (
          <>
            <label className="check">
              <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
              <span>I have saved my Emergency Kit</span>
            </label>
            <button className="btn btn-brass" type="button" disabled={!saved} onClick={onDone}>
              Continue
            </button>
          </>
        )}
      </div>
    </div>
  );
}

import { useEffect, useState } from "react";
import menuLogins from "../../assets/shots/menu-logins.png";
import { useI18n } from "../../i18n/context";
import { Icon } from "../Icon";
import { Scramble } from "./Scramble";

/* Demo values. The Secret Key uses the real format (H1 + 8 groups of 4) but
   is made up; the ciphertext is shaped like the real blobs, not produced by
   them. */
const SECRET_KEY = "H1-7QKP-2M9X-R4TD-8WVN";

export function KeysScene({ active }: { active: boolean }) {
  const k = useI18n().t.scenes.keys;
  return (
    <div className={`scene keys ${active ? "is-active" : ""}`}>
      <div className="keys__inputs">
        <div className="keys__input">
          <span className="keys__label">{k.masterLabel}</span>
          <span className="keys__value keys__value--masked">••••••••••••••</span>
          <span className="keys__where">{k.masterWhere}</span>
        </div>
        <div className="keys__input">
          <span className="keys__label">{k.secretLabel}</span>
          <span className="keys__value">{SECRET_KEY}…</span>
          <span className="keys__where">{k.secretWhere}</span>
        </div>
      </div>
      <div className="keys__flow">
        <div className="keys__step" style={{ ["--d" as string]: "0ms" }}>
          <span className="keys__op">Argon2id</span>
          <span className="keys__param">{k.argonParam}</span>
        </div>
        <div className="keys__step" style={{ ["--d" as string]: "180ms" }}>
          <span className="keys__op">HKDF-SHA-256</span>
          <span className="keys__param">{k.hkdfParam}</span>
        </div>
        <div className="keys__step keys__step--out" style={{ ["--d" as string]: "360ms" }}>
          <Icon name="key" size={18} />
          <span className="keys__op">{k.vaultKey}</span>
          <span className="keys__param">{k.vaultParam}</span>
        </div>
      </div>
    </div>
  );
}

/* Plaintext values in the order of the localized labels; the last one (the
   one-time code) is a word, so it comes from the dictionary. */
const FIELDS = [
  { plain: "Fernway", cipher: "9f3a1c07e2" },
  { plain: "sam@example.com", cipher: "b41d7e09a3f25c88e1" },
  { plain: "••••••••••••", cipher: "5ce0ab19d7f4630b27" },
  { plain: "fernway.example", cipher: "e7082fd4c19b3a" },
  { plain: null, cipher: "0dc3a95e71" },
];

export function SealScene({ active }: { active: boolean }) {
  const t = useI18n().t.scenes.seal;
  const [sealed, setSealed] = useState(false);
  useEffect(() => {
    if (!active) {
      setSealed(false);
      return;
    }
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const t = window.setTimeout(() => setSealed(true), reduced ? 0 : 650);
    return () => window.clearTimeout(t);
  }, [active]);

  return (
    <div className={`scene seal ${active ? "is-active" : ""} ${sealed ? "is-sealed" : ""}`}>
      <div className="seal__card">
        <div className="seal__head">
          <span className="seal__avatar">F</span>
          <span className="seal__state">{sealed ? t.sealed : t.plaintext}</span>
          <Icon name="lock" size={16} className="seal__lock" />
        </div>
        <dl className="seal__fields">
          {FIELDS.map((f, i) => (
            <div key={i}>
              <dt>{t.labels[i]}</dt>
              <dd>
                <Scramble plain={f.plain ?? t.totpPlain} cipher={f.cipher} sealed={sealed} />
              </dd>
            </div>
          ))}
        </dl>
      </div>
      <p className="seal__note">
        {t.notes.map((n) => (
          <span key={n}>{n}</span>
        ))}
      </p>
    </div>
  );
}

const BLOBS = ["9f3a1c07e2b41d7e", "5ce0ab19d7f4630b", "e7082fd4c19b3a0d", "c3a95e71a0f6b288", "41d0e9f2ab3c7715"];

export function StoreScene({ active }: { active: boolean }) {
  const t = useI18n().t.scenes.store;
  return (
    <div className={`scene store ${active ? "is-active" : ""}`}>
      <div className="store__server">
        <div className="store__title">
          <Icon name="server" size={18} />
          <span>
            <strong>havenkeys-server</strong>
            <em>{t.where}</em>
          </span>
        </div>
        <ul className="store__blobs" aria-label={t.blobsAria}>
          {BLOBS.map((b) => (
            <li key={b}>{b}…</li>
          ))}
        </ul>
        <div className="store__sees">
          <p>
            <Icon name="cross" size={14} /> {t.cantOpen}
          </p>
          <p>
            <Icon name="check" size={14} /> {t.doesSee}
          </p>
        </div>
      </div>
      <div className="store__devices">
        <div className="store__device">
          <Icon name="laptop" size={18} />
          <span>
            <strong>{t.laptop}</strong>
            <em>{t.deviceNote}</em>
          </span>
          <i className="store__packet" aria-hidden="true" />
        </div>
        <div className="store__device">
          <Icon name="monitor" size={18} />
          <span>
            <strong>{t.desktop}</strong>
            <em>{t.deviceNote}</em>
          </span>
          <i className="store__packet store__packet--late" aria-hidden="true" />
        </div>
      </div>
    </div>
  );
}

const ORIGINS = [
  { host: "fernway.example", ok: true },
  { host: "accounts.fernway.example", ok: true },
  { host: "fernway.example.evil.com", ok: false },
  { host: "fernway-login.example", ok: false },
];

export function FillScene({ active }: { active: boolean }) {
  const t = useI18n().t.scenes.fill;
  return (
    <div className={`scene fill ${active ? "is-active" : ""}`}>
      <div className="fill__demo">
        <span className="fill__label">{t.emailLabel}</span>
        <div className="fill__field" aria-hidden="true">
          <span className="fill__caret" />
        </div>
        <img
          className="fill__menu"
          src={menuLogins}
          width={340}
          height={136}
          alt={t.menuAlt}
        />
      </div>
      <ul className="fill__origins">
        {ORIGINS.map((o, i) => (
          <li key={o.host} className={o.ok ? "is-ok" : "is-denied"}>
            <Icon name={o.ok ? "check" : "cross"} size={15} />
            <code>{o.host}</code>
            <span>{t.origins[i]}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

export const SCENES = [KeysScene, SealScene, StoreScene, FillScene];

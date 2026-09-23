import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import menuLogins from "../../assets/shots/menu-logins.png";
import menuOtp from "../../assets/shots/menu-otp.png";
import menuGenerate from "../../assets/shots/menu-generate.png";
import saveAdd from "../../assets/shots/save-add.png";
import popup from "../../assets/shots/popup.png";
import { useI18n } from "../../i18n/context";
import type { SceneId } from "../../i18n/en";
import { Mark } from "../Mark";

/* On phones the whole demo page would scale past legibility, so the
   extension UI itself is shown at its real size instead. */
const SOLO: Record<SceneId, { src: string; w: number; h: number }> = {
  signin: { src: menuLogins, w: 340, h: 136 },
  otp: { src: menuOtp, w: 340, h: 90 },
  signup: { src: menuGenerate, w: 340, h: 90 },
  save: { src: saveAdd, w: 340, h: 138 },
  popup: { src: popup, w: 320, h: 251 },
};

const SCENARIOS: Array<{ id: SceneId; path: string }> = [
  { id: "signin", path: "/signin" },
  { id: "otp", path: "/verify" },
  { id: "signup", path: "/signup" },
  { id: "save", path: "/home" },
  { id: "popup", path: "/verify" },
];

/* The demo page is drawn at a fixed size and scaled to fit, so the real
   screenshots keep their proportions against it at every width. */
const PAGE_W = 960;
const PAGE_H = 580;

function useFitScale() {
  const ref = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      if (entry) setScale(Math.min(1, entry.contentRect.width / PAGE_W));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return { ref, scale };
}

function Field({ label, value, focused, placeholder }: { label: string; value?: string; focused?: boolean; placeholder?: string }) {
  return (
    <div className="fw-field">
      <span className="fw-label">{label}</span>
      <span className={`fw-input ${focused ? "is-focused" : ""}`}>
        {value ?? <span className="fw-placeholder">{placeholder}</span>}
      </span>
    </div>
  );
}

function DemoCard({ scene }: { scene: SceneId }) {
  const { t } = useI18n();
  const d = t.showcase.demo;
  const alt = (id: SceneId) => t.showcase.scenarios[id].alt;
  switch (scene) {
    case "signin":
      return (
        <div className="fw-card">
          <h4>{d.signIn}</h4>
          <p>{d.welcomeBack}</p>
          <Field label={d.email} focused placeholder="you@example.com" />
          <div className="fw-anchor">
            <img className="fw-overlay fw-overlay--field" src={menuLogins} width={340} height={136} alt={alt("signin")} />
          </div>
          <Field label={d.password} placeholder="" />
          <span className="fw-button">{d.continue}</span>
        </div>
      );
    case "otp":
    case "popup":
      return (
        <div className="fw-card">
          <h4>{d.twoStep}</h4>
          <p>{d.enterCode}</p>
          <Field label={d.verificationCode} focused placeholder="123 456" />
          {scene === "otp" && (
            <div className="fw-anchor">
              <img className="fw-overlay fw-overlay--field" src={menuOtp} width={340} height={90} alt={alt("otp")} />
            </div>
          )}
          <span className="fw-button">{d.verify}</span>
        </div>
      );
    case "signup":
      return (
        <div className="fw-card">
          <h4>{d.createTitle}</h4>
          <p>{d.createLede}</p>
          <Field label={d.email} value="sam@example.com" />
          <Field label={d.newPassword} focused placeholder="" />
          <div className="fw-anchor">
            <img className="fw-overlay fw-overlay--field" src={menuGenerate} width={340} height={90} alt={alt("signup")} />
          </div>
          <span className="fw-button">{d.createButton}</span>
        </div>
      );
    case "save":
      return (
        <div className="fw-card">
          <h4>{d.welcomeSam}</h4>
          <p>{d.signedIn}</p>
          <span className="fw-button fw-button--soft">{d.dashboard}</span>
        </div>
      );
  }
}

export function BrowserShowcase() {
  const { t } = useI18n();
  const [index, setIndex] = useState(0);
  const tabs = useRef<Array<HTMLButtonElement | null>>([]);
  const { ref, scale } = useFitScale();
  const scenario = SCENARIOS[index]!;
  const copy = t.showcase.scenarios[scenario.id];

  useEffect(() => {
    // Warm the cache so switching tabs never shows a half-loaded frame.
    for (const src of [menuOtp, menuGenerate, saveAdd, popup]) new Image().src = src;
  }, []);

  function onKey(e: KeyboardEvent<HTMLDivElement>) {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const next = (index + (e.key === "ArrowRight" ? 1 : SCENARIOS.length - 1)) % SCENARIOS.length;
    setIndex(next);
    tabs.current[next]?.focus();
  }

  return (
    <div className="showcase">
      <div className="showcase__tabs" role="tablist" aria-label={t.showcase.tabsAria} onKeyDown={onKey}>
        {SCENARIOS.map((s, i) => (
          <button
            key={s.id}
            ref={(el) => {
              tabs.current[i] = el;
            }}
            role="tab"
            type="button"
            id={`tab-${s.id}`}
            aria-selected={i === index}
            aria-controls="showcase-panel"
            tabIndex={i === index ? 0 : -1}
            onClick={() => setIndex(i)}
          >
            {t.showcase.scenarios[s.id].tab}
          </button>
        ))}
      </div>

      <div className="showcase__body" role="tabpanel" id="showcase-panel" aria-labelledby={`tab-${scenario.id}`}>
        <div className="showcase__copy" key={scenario.id}>
          <h3>{copy.title}</h3>
          <p>{copy.body}</p>
        </div>

        <figure className="showcase__solo" key={`solo-${scenario.id}`}>
          <img src={SOLO[scenario.id].src} width={SOLO[scenario.id].w} height={SOLO[scenario.id].h} alt={copy.alt} />
          <figcaption>{t.showcase.soloCaption(<code>accounts.fernway.example{scenario.path}</code>)}</figcaption>
        </figure>

        <div className="showcase__frame" ref={ref} style={{ height: PAGE_H * scale + 44 }}>
          <div className="browser" style={{ width: PAGE_W, transform: `scale(${scale})` }}>
            <div className="browser__chrome" aria-hidden="true">
              <div className="browser__tab">
                <i />
                Fernway
              </div>
              <div className="browser__bar">
                <span className="browser__url">
                  <b>accounts.fernway.example</b>
                  {scenario.path}
                </span>
                <span className={`browser__ext ${scenario.id === "popup" ? "is-open" : ""}`}>
                  <Mark size={16} tile={false} />
                </span>
              </div>
            </div>
            <div className="browser__page" style={{ height: PAGE_H - 44 }}>
              <div className="fw-top">
                <span className="fw-logo" />
                Fernway
                <span className="fw-demo">{t.showcase.demo.badge}</span>
              </div>
              <DemoCard scene={scenario.id} />
              {scenario.id === "save" && (
                <img className="fw-overlay fw-overlay--corner" src={saveAdd} width={340} height={138} alt={t.showcase.scenarios.save.alt} />
              )}
              {scenario.id === "popup" && (
                <img className="fw-overlay fw-overlay--popup" src={popup} width={320} height={251} alt={t.showcase.scenarios.popup.alt} />
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

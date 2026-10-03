import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { GeneratedPassword, GeneratorOptions } from "../lib/types";
import { strengthLevel } from "../lib/format";
import { Icon } from "../components/Icon";
import { Switch } from "../components/Switch";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

const initial: GeneratorOptions = {
  length: 24,
  uppercase: true,
  lowercase: true,
  digits: true,
  symbols: true,
  avoidAmbiguous: false,
};

/** The label for each comes from the messages, under the same key. */
const toggles: Array<{ key: keyof Omit<GeneratorOptions, "length">; hint: string }> = [
  { key: "uppercase", hint: "A–Z" },
  { key: "lowercase", hint: "a–z" },
  { key: "digits", hint: "0–9" },
  { key: "symbols", hint: "!@#…" },
  { key: "avoidAmbiguous", hint: "l, 1, O, 0" },
];

/** Colour digits and symbols differently so a password is easier to read out. */
function Characters({ value }: { value: string }) {
  return (
    <>
      {[...value].map((c, i) => (
        <span key={i} className={/[0-9]/.test(c) ? "ch-digit" : /[A-Za-z]/.test(c) ? undefined : "ch-symbol"}>
          {c}
        </span>
      ))}
    </>
  );
}

export function GeneratorView() {
  const toast = useToast();
  const { t } = useI18n();
  const [options, setOptions] = useState(initial);
  const [result, setResult] = useState<GeneratedPassword | null>(null);
  // The failure itself, so its text follows a language change.
  const [error, setError] = useState<{ cause: unknown } | null>(null);

  // The policy last read from or written to the vault's settings, so the
  // values loaded on open are not written straight back.
  const saved = useRef<string | null>(null);

  useEffect(() => {
    let live = true;
    api.getSettings().then(
      (s) => {
        if (!live) return;
        saved.current = JSON.stringify(s.generator);
        setOptions(s.generator);
      },
      // Unreadable: keep the default and do not save over what is there.
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  // Save each change (debounced for the slider). The browser extension's
  // "Generate strong password" uses this policy too.
  useEffect(() => {
    const json = JSON.stringify(options);
    if (saved.current === null || saved.current === json) return;
    if (!(options.uppercase || options.lowercase || options.digits || options.symbols)) return;
    const id = setTimeout(() => {
      api.setGeneratorOptions(options).then(
        () => (saved.current = json),
        (e) => toast(errorMessage(e, t, t.generator.saveFailed), "error"),
      );
    }, 400);
    return () => clearTimeout(id);
  }, [options, toast, t]);

  const regenerate = useCallback(async (opts: GeneratorOptions) => {
    try {
      setResult(await api.generate(opts));
      setError(null);
    } catch (e) {
      setResult(null);
      setError({ cause: e });
    }
  }, []);

  useEffect(() => {
    void regenerate(options);
  }, [options, regenerate]);

  // Drop the generated value when leaving this screen.
  useEffect(() => () => setResult(null), []);

  async function copy() {
    if (!result) return;
    try {
      const r = await api.copyGenerated(result.password);
      toast(t.generator.copied(r.clearAfterSeconds));
    } catch (e) {
      toast(errorMessage(e, t, t.common.couldNotCopy), "error");
    }
  }

  const level = result ? strengthLevel(result.entropyBits) : null;

  const bits = result ? Math.round(result.entropyBits) : 0;

  return (
    <section className="tool" aria-labelledby="gen-title">
      <header className="tool-head" data-tauri-drag-region>
        <h2 id="gen-title">{t.generator.title}</h2>
        <p className="tool-lede">{t.generator.lede}</p>
      </header>

      <div className="gen-output">
        <p className="mono gen-password" aria-live="polite">
          {result ? (
            <span className="gen-chars" key={result.password}>
              <Characters value={result.password} />
            </span>
          ) : (
            <span className="muted">
              {error && errorMessage(error.cause, t, t.generator.failed)}
            </span>
          )}
        </p>
        <div className="gen-foot">
          {result && level ? (
            <div className={`strength strength-${level}`}>
              <div className="strength-bar">
                <span style={{ transform: `scaleX(${Math.min(1, result.entropyBits / 128)})` }} />
              </div>
              <span>
                <strong>{t.generator.strength[level]}</strong> · {t.generator.aboutBits(bits)}
              </span>
            </div>
          ) : (
            <span />
          )}
          <div className="gen-actions">
            <button className="btn" onClick={() => void regenerate(options)}>
              <Icon name="refresh" size={16} /> {t.generator.regenerate}
            </button>
            <button className="btn btn-primary" onClick={() => void copy()} disabled={!result}>
              <Icon name="copy" size={16} /> {t.generator.copy}
            </button>
          </div>
        </div>
      </div>

      <h3 className="group-title">{t.generator.length}</h3>
      <div className="group">
        <label className="row row-slider">
          <span className="sr-only">{t.generator.length}</span>
          <input
            type="range"
            min={8}
            max={128}
            value={options.length}
            style={{ ["--fill" as string]: `${((options.length - 8) / 120) * 100}%` }}
            onChange={(e) => setOptions((o) => ({ ...o, length: Number(e.target.value) }))}
          />
          <strong className="gen-length">{options.length}</strong>
        </label>
      </div>

      <h3 className="group-title">{t.generator.characters}</h3>
      <div className="group">
        {toggles.map((toggle) => (
          <div key={toggle.key} className="row">
            <span className="row-label-inline">
              {t.generator[toggle.key]} <span className="muted">{toggle.hint}</span>
            </span>
            <Switch
              label={t.generator[toggle.key]}
              checked={options[toggle.key]}
              onChange={(checked) => setOptions((o) => ({ ...o, [toggle.key]: checked }))}
            />
          </div>
        ))}
      </div>
    </section>
  );
}

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { GeneratedPassword, GeneratorOptions } from "../lib/types";
import { strengthLabel } from "../lib/format";
import { Icon } from "../components/Icon";
import { Switch } from "../components/Switch";
import { useToast } from "../components/Toast";

const initial: GeneratorOptions = {
  length: 24,
  uppercase: true,
  lowercase: true,
  digits: true,
  symbols: true,
  avoidAmbiguous: false,
};

const toggles: Array<{ key: keyof Omit<GeneratorOptions, "length">; label: string; hint: string }> = [
  { key: "uppercase", label: "Uppercase", hint: "A–Z" },
  { key: "lowercase", label: "Lowercase", hint: "a–z" },
  { key: "digits", label: "Numbers", hint: "0–9" },
  { key: "symbols", label: "Symbols", hint: "!@#…" },
  { key: "avoidAmbiguous", label: "Avoid look-alikes", hint: "l, 1, O, 0" },
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
  const [options, setOptions] = useState(initial);
  const [result, setResult] = useState<GeneratedPassword | null>(null);
  const [error, setError] = useState<string | null>(null);

  const regenerate = useCallback(async (opts: GeneratorOptions) => {
    try {
      setResult(await api.generate(opts));
      setError(null);
    } catch (e) {
      setResult(null);
      setError(e instanceof ApiError ? e.message : "Could not generate a password.");
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
      toast(`Password copied. The clipboard clears in ${r.clearAfterSeconds} s.`);
    } catch (e) {
      toast(e instanceof ApiError ? e.message : "Could not copy.", "error");
    }
  }

  const label = result ? strengthLabel(result.entropyBits) : null;

  const bits = result ? Math.round(result.entropyBits) : 0;

  return (
    <section className="tool" aria-labelledby="gen-title">
      <header className="tool-head" data-tauri-drag-region>
        <h2 id="gen-title">Password generator</h2>
        <p className="tool-lede">
          Made on this computer from the operating system’s secure random source. Nothing is saved until you use it.
        </p>
      </header>

      <div className="gen-output">
        <p className="mono gen-password" aria-live="polite">
          {result ? (
            <span className="gen-chars" key={result.password}>
              <Characters value={result.password} />
            </span>
          ) : (
            <span className="muted">{error}</span>
          )}
        </p>
        <div className="gen-foot">
          {result && label ? (
            <div className={`strength strength-${label.toLowerCase()}`}>
              <div className="strength-bar">
                <span style={{ transform: `scaleX(${Math.min(1, result.entropyBits / 128)})` }} />
              </div>
              <span>
                <strong>{label}</strong> · about {bits} bits
              </span>
            </div>
          ) : (
            <span />
          )}
          <div className="gen-actions">
            <button className="btn" onClick={() => void regenerate(options)}>
              <Icon name="refresh" size={16} /> Regenerate
            </button>
            <button className="btn btn-primary" onClick={() => void copy()} disabled={!result}>
              <Icon name="copy" size={16} /> Copy
            </button>
          </div>
        </div>
      </div>

      <h3 className="group-title">Length</h3>
      <div className="group">
        <label className="row row-slider">
          <span className="sr-only">Length</span>
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

      <h3 className="group-title">Characters</h3>
      <div className="group">
        {toggles.map((t) => (
          <div key={t.key} className="row">
            <span className="row-label-inline">
              {t.label} <span className="muted">{t.hint}</span>
            </span>
            <Switch
              label={t.label}
              checked={options[t.key]}
              onChange={(checked) => setOptions((o) => ({ ...o, [t.key]: checked }))}
            />
          </div>
        ))}
      </div>
    </section>
  );
}

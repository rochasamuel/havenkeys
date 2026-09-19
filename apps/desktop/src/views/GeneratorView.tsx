import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { GeneratedPassword, GeneratorOptions } from "../lib/types";
import { strengthLabel } from "../lib/format";
import { Icon } from "../components/Icon";
import { useToast } from "../components/Toast";

const initial: GeneratorOptions = {
  length: 24,
  uppercase: true,
  lowercase: true,
  digits: true,
  symbols: true,
  avoidAmbiguous: false,
};

const toggles: Array<{ key: keyof Omit<GeneratorOptions, "length">; label: string }> = [
  { key: "uppercase", label: "Uppercase (A–Z)" },
  { key: "lowercase", label: "Lowercase (a–z)" },
  { key: "digits", label: "Numbers (0–9)" },
  { key: "symbols", label: "Symbols (!@#…)" },
  { key: "avoidAmbiguous", label: "Avoid look-alikes (l, 1, O, 0)" },
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

  return (
    <section className="tool" aria-labelledby="gen-title">
      <h2 id="gen-title">Password generator</h2>
      <p className="tool-lede">Generated on this computer from the operating system’s secure random source. Nothing is saved until you use it.</p>

      <div className="gen-output">
        <p className="mono gen-password" aria-live="polite">
          {result ? <Characters value={result.password} /> : <span className="muted">{error}</span>}
        </p>
        <div className="gen-actions">
          <button className="btn" onClick={() => void regenerate(options)}>
            <Icon name="refresh" size={16} /> Regenerate
          </button>
          <button className="btn btn-primary" onClick={() => void copy()} disabled={!result}>
            <Icon name="copy" size={16} /> Copy
          </button>
        </div>
      </div>

      {result && label && (
        <div className={`strength strength-${label.toLowerCase()}`}>
          <div className="strength-bar">
            <span style={{ width: `${Math.min(100, (result.entropyBits / 128) * 100)}%` }} />
          </div>
          <span>
            {label}, about {Math.round(result.entropyBits)} bits of entropy
          </span>
        </div>
      )}

      <div className="gen-options">
        <label className="control">
          <span>
            Length <strong className="gen-length">{options.length}</strong>
          </span>
          <input
            type="range"
            min={8}
            max={128}
            value={options.length}
            onChange={(e) => setOptions((o) => ({ ...o, length: Number(e.target.value) }))}
          />
        </label>
        <fieldset className="checks">
          <legend>Characters</legend>
          {toggles.map((t) => (
            <label key={t.key} className="check">
              <input
                type="checkbox"
                checked={options[t.key]}
                onChange={(e) => setOptions((o) => ({ ...o, [t.key]: e.target.checked }))}
              />
              <span>{t.label}</span>
            </label>
          ))}
        </fieldset>
      </div>
    </section>
  );
}

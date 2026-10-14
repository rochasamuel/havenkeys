import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { api, ApiError } from "../lib/api";
import type { QuickAccessStatus } from "../lib/types";
import { formatChord, KEY_REFUSED, recordKey, shortcutPlatform } from "../lib/shortcut";
import { Switch } from "../components/Switch";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

/**
 * Settings → Quick Access: the switch, the chord with Change…/Reset, and
 * the recorder. Rust registers whatever is chosen (quick_shortcut.rs); this
 * only proposes a chord and shows the answer.
 */
export function QuickAccessSection() {
  const toast = useToast();
  const { t } = useI18n();
  const platform = shortcutPlatform();
  const [status, setStatus] = useState<QuickAccessStatus | null>(null);
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);
  /** Why the last proposal was not taken; cleared on start, cancel and success. */
  const [problem, setProblem] = useState<string | null>(null);
  const recorderRef = useRef<HTMLInputElement>(null);
  const changeRef = useRef<HTMLButtonElement>(null);
  const wasRecording = useRef(false);
  /** Set synchronously so a second key press before React re-renders is ignored. */
  const sending = useRef(false);

  useEffect(() => {
    // Unknown (null) hides the block: the OS may not support it.
    api
      .quickAccessStatus()
      .then(setStatus)
      .catch(() => setStatus(null));
  }, []);

  useEffect(() => {
    if (recording) {
      recorderRef.current?.focus();
    } else if (wasRecording.current && (!document.activeElement || document.activeElement === document.body)) {
      // Esc or an accepted chord removed the focused recorder: hand focus
      // back to Change…, not to the top of the page. A click elsewhere keeps
      // the focus it moved.
      changeRef.current?.focus();
    }
    wasRecording.current = recording;
  }, [recording]);

  async function updateEnabled(enabled: boolean) {
    setRecording(false);
    setProblem(null);
    try {
      setStatus(await api.setQuickAccess(enabled));
      toast(t.settings.saved);
    } catch (e) {
      toast(errorMessage(e, t, t.settings.saveFailed), "error");
    }
  }

  function startRecording() {
    setProblem(null);
    setRecording(true);
  }

  function stopRecording() {
    setRecording(false);
    setProblem(null);
  }

  async function propose(keys: string | null) {
    if (sending.current) return;
    sending.current = true;
    setBusy(true);
    try {
      const next = await api.setQuickAccessShortcut(keys);
      setStatus(next);
      if (next.candidateInUse) {
        setProblem(t.settings.quickAccessCandidateInUse);
        return;
      }
      setProblem(null);
      setRecording(false);
      toast(t.settings.saved);
    } catch (e) {
      setProblem(errorMessage(e, t, t.settings.saveFailed));
    } finally {
      sending.current = false;
      setBusy(false);
    }
  }

  function onRecorderKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    // Every key belongs to the recorder: Cmd/Ctrl+L must not lock the vault,
    // Tab must not move focus.
    e.preventDefault();
    e.stopPropagation();
    if (sending.current) return;
    const r = recordKey(e.nativeEvent);
    switch (r.kind) {
      case "wait":
        return;
      case "cancel":
        stopRecording();
        return;
      case "unusable":
        setProblem(errorMessage(new ApiError("quick_access_shortcut_invalid", KEY_REFUSED), t));
        return;
      case "chord":
        void propose(r.keys);
    }
  }

  if (!status) return null;
  const chord = formatChord(status.keys, platform);

  return (
    <div className="settings-block">
      <h3 className="group-title">{t.settings.quickAccess}</h3>
      <div className="group">
        <div className="row">
          <span className="row-label-inline">{t.settings.quickAccessShortcut}</span>
          <Switch label={t.settings.quickAccessShortcutLabel} checked={status.enabled} onChange={(checked) => void updateEnabled(checked)} />
        </div>
        {status.enabled && (
          <div className="row quick-shortcut-row">
            <span className="row-label-inline">{t.settings.quickAccessKeys}</span>
            {recording ? (
              <input
                ref={recorderRef}
                className="shortcut-recorder"
                readOnly
                value=""
                placeholder={t.settings.quickAccessRecording}
                aria-label={t.settings.quickAccessRecordLabel}
                aria-busy={busy}
                onKeyDown={onRecorderKeyDown}
                onBlur={() => {
                  if (!sending.current) stopRecording();
                }}
              />
            ) : (
              <>
                <kbd className="shortcut-keys">{chord}</kbd>
                <button ref={changeRef} type="button" className="btn btn-small btn-ghost" onClick={startRecording}>
                  {t.settings.quickAccessChange}
                </button>
                {!status.isDefault && (
                  <button type="button" className="btn btn-small btn-quiet" disabled={busy} onClick={() => void propose(null)}>
                    {t.settings.quickAccessReset}
                  </button>
                )}
              </>
            )}
          </div>
        )}
      </div>
      {problem && (
        <p className="group-note warn" role="alert">
          {problem}
        </p>
      )}
      {status.enabled && status.inUse && !recording && (
        <div className="group-note warn quick-in-use">
          <span role="status">{t.settings.quickAccessInUse}</span>
          <button type="button" className="btn btn-small btn-ghost" onClick={startRecording}>
            {t.settings.quickAccessChooseAnother}
          </button>
        </div>
      )}
      <p className="group-note">{t.settings.quickAccessNote(chord)}</p>
    </div>
  );
}

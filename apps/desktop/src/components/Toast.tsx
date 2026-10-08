import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { Icon } from "./Icon";

type Tone = "info" | "error";
/** One button on the toast, such as Undo. Clicking it closes the toast. */
interface ToastAction {
  label: string;
  onClick: () => void;
}
interface ToastState {
  id: number;
  text: string;
  tone: Tone;
  action?: ToastAction;
}

const ToastContext = createContext<(text: string, tone?: Tone, action?: ToastAction) => void>(() => undefined);

export function useToast() {
  return useContext(ToastContext);
}

/** Short status messages. Never pass secrets here. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<ToastState | null>(null);
  const [leaving, setLeaving] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const exit = useRef<number | undefined>(undefined);

  const show = useCallback((text: string, tone: Tone = "info", action?: ToastAction) => {
    window.clearTimeout(timer.current);
    window.clearTimeout(exit.current);
    setLeaving(false);
    setToast({ id: Date.now(), text, tone, action });
    // An action stays longer, so there is time to reach it.
    timer.current = window.setTimeout(
      () => {
        setLeaving(true);
        exit.current = window.setTimeout(() => setToast(null), 220);
      },
      action ? 6000 : 3200,
    );
  }, []);

  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
      window.clearTimeout(exit.current);
    },
    [],
  );

  return (
    <ToastContext.Provider value={show}>
      {children}
      <div className="toast-region" role="status" aria-live="polite">
        {toast && (
          <div key={toast.id} className={`toast toast-${toast.tone}${leaving ? " is-leaving" : ""}`}>
            <Icon name={toast.tone === "error" ? "alert" : "check"} size={15} />
            <span>{toast.text}</span>
            {toast.action && (
              <button
                type="button"
                className="toast-action"
                onClick={() => {
                  toast.action?.onClick();
                  window.clearTimeout(timer.current);
                  window.clearTimeout(exit.current);
                  setToast(null);
                }}
              >
                {toast.action.label}
              </button>
            )}
          </div>
        )}
      </div>
    </ToastContext.Provider>
  );
}

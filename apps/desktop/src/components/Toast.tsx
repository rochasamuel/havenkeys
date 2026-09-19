import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";

type Tone = "info" | "error";
interface ToastState {
  id: number;
  text: string;
  tone: Tone;
}

const ToastContext = createContext<(text: string, tone?: Tone) => void>(() => undefined);

export function useToast() {
  return useContext(ToastContext);
}

/** Short status messages. Never pass secrets here. */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<ToastState | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const show = useCallback((text: string, tone: Tone = "info") => {
    window.clearTimeout(timer.current);
    setToast({ id: Date.now(), text, tone });
    timer.current = window.setTimeout(() => setToast(null), 3200);
  }, []);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return (
    <ToastContext.Provider value={show}>
      {children}
      <div className="toast-region" role="status" aria-live="polite">
        {toast && (
          <div key={toast.id} className={`toast toast-${toast.tone}`}>
            {toast.text}
          </div>
        )}
      </div>
    </ToastContext.Provider>
  );
}

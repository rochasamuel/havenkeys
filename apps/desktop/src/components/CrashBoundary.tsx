import { Component, type ErrorInfo, type ReactNode } from "react";
import { useI18n } from "../i18n/context";
import { Seal } from "./Seal";

/**
 * What a crash shows instead of a blank window: a way back (Reload) and a
 * short description to report. Only the error's type and message and the
 * names of the components it went through; never props or state, so no
 * vault data. Nothing is logged or sent anywhere.
 */
export class CrashBoundary extends Component<{ children: ReactNode }, { details: string | null }> {
  state = { details: null as string | null };

  static getDerivedStateFromError(error: unknown) {
    const e = error instanceof Error ? error : new Error("Unknown error");
    return { details: `${e.name}: ${e.message}`.slice(0, MAX_MESSAGE) };
  }

  componentDidCatch(_error: unknown, info: ErrorInfo) {
    const names = (info.componentStack ?? "")
      .split("\n")
      .map((line) => /at (\w+)/.exec(line)?.[1])
      .filter(Boolean)
      .slice(0, MAX_COMPONENTS);
    if (names.length) this.setState((s) => ({ details: `${s.details}\n${names.join(" < ")}` }));
  }

  render() {
    return this.state.details === null ? this.props.children : <CrashScreen details={this.state.details} />;
  }
}

const MAX_MESSAGE = 300;
const MAX_COMPONENTS = 8;

function CrashScreen({ details }: { details: string }) {
  const { t } = useI18n();
  return (
    <div className="app-shell">
      <main className="welcome" role="alert">
        <div className="welcome-card">
          <header className="welcome-head">
            <Seal />
            <h1>{t.app.crashTitle}</h1>
          </header>
          <p className="fatal-message">{t.app.crashBody}</p>
          <pre className="crash-details selectable">{details}</pre>
          <button type="button" className="btn btn-primary btn-lg" onClick={() => window.location.reload()}>
            {t.app.crashReload}
          </button>
        </div>
      </main>
    </div>
  );
}

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/hanken-grotesk";
import "@fontsource-variable/source-serif-4/opsz.css";
import "@fontsource-variable/source-serif-4/opsz-italic.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
// Tokens first: styles.css and every component below read them as
// custom properties, so they must be declared before anything uses them.
import "@havenkeys/ui/tokens.css";
import "./styles.css";
import { App } from "./App";
import { ToastProvider } from "./components/Toast";
import { applyTheme } from "./lib/theme";

applyTheme("dark");
// macOS draws its window buttons over the sidebar (titleBarStyle "Overlay"),
// so the layout leaves room for them there and nowhere else.
if (/Mac/.test(navigator.userAgent)) document.documentElement.dataset.platform = "mac";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <ToastProvider>
        <App />
      </ToastProvider>
    </StrictMode>,
  );
}

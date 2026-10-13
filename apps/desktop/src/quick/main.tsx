import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/hanken-grotesk";
import "@fontsource-variable/source-serif-4/opsz.css";
import "@fontsource/jetbrains-mono/400.css";
import "@havenkeys/ui/tokens.css";
import "../styles.css";
import "./quick.css";
import { I18nProvider } from "../i18n/context";
import { applyTheme } from "../lib/theme";
import { QuickApp } from "./QuickApp";

// The theme is a sealed vault setting this window cannot read
// (capabilities/quick.json has no get_settings): it stays the default.
applyTheme("dark");
if (/Mac/.test(navigator.userAgent)) document.documentElement.dataset.platform = "mac";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <I18nProvider syncTray={false}>
        <QuickApp />
      </I18nProvider>
    </StrictMode>,
  );
}

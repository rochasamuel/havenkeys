import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/hanken-grotesk";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "./styles.css";
import { App } from "./App";
import { ToastProvider } from "./components/Toast";
import { applyTheme } from "./lib/theme";

applyTheme("dark");

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

import { useEffect } from "react";
import { BrowserRouter, Routes, Route, useLocation, useNavigate } from "react-router-dom";
import { Analytics } from "@vercel/analytics/react";
import { Nav } from "./components/Nav";
import { Footer } from "./components/Footer";
import { ScrollToTop } from "./components/ScrollToTop";
import { Home } from "./pages/Home";
import { Download } from "./pages/Download";
import { Security } from "./pages/Security";
import { Privacy } from "./pages/Privacy";
import { Terms } from "./pages/Terms";
import { NotFound } from "./pages/NotFound";
import { I18nProvider } from "./i18n/context";
import { LOCALES, localeFromPath, prefersPortuguese } from "./i18n/locale";
import { readPreference } from "./i18n/preference";

const PAGES = [
  { path: "", element: <Home /> },
  { path: "download", element: <Download /> },
  { path: "security", element: <Security /> },
  { path: "privacy", element: <Privacy /> },
  { path: "terms", element: <Terms /> },
];

/*
 * A Portuguese-speaking browser landing on the English homepage is sent to
 * /pt-br once, unless the visitor has already picked a language. Only the
 * bare homepage redirects: a shared link to an English page stays English.
 */
function LanguageRedirect() {
  const { pathname, hash } = useLocation();
  const navigate = useNavigate();
  useEffect(() => {
    if (pathname !== "/") return;
    if (readPreference() !== null) return;
    if (prefersPortuguese(navigator.languages ?? [navigator.language])) {
      navigate(LOCALES["pt-BR"].prefix + hash, { replace: true });
    }
    // Only on first load: later visits to "/" are deliberate.
  }, []);
  return null;
}

function Site() {
  const { pathname } = useLocation();
  const locale = localeFromPath(pathname);
  const pt = LOCALES["pt-BR"].prefix;
  return (
    <I18nProvider locale={locale}>
      <LanguageRedirect />
      <ScrollToTop />
      <Nav />
      <main>
        <Routes>
          {PAGES.map((p) => (
            <Route key={`en-${p.path}`} path={`/${p.path}`} element={p.element} />
          ))}
          {PAGES.map((p) => (
            <Route key={`pt-${p.path}`} path={p.path ? `${pt}/${p.path}` : pt} element={p.element} />
          ))}
          <Route path="*" element={<NotFound />} />
        </Routes>
      </main>
      <Footer />
    </I18nProvider>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <Site />
      <Analytics />
    </BrowserRouter>
  );
}

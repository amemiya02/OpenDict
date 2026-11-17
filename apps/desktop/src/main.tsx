import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { I18nProvider, pickLocale } from "./i18n";
import "./styles.css";

const locale = pickLocale(navigator.languages);
document.documentElement.lang = locale;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <I18nProvider locale={locale}>
      <App />
    </I18nProvider>
  </StrictMode>,
);

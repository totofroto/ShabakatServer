import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import App from "./App";
import { ErrorBoundary } from "./components/ui/error-boundary";
import { LanguageProvider } from "./context/LanguageContext";
import { NotificationCenterProvider } from "./context/NotificationCenterContext";
import { NetworkConnectivityProvider } from "./context/NetworkConnectivityContext";
import { ScanProvider } from "./context/ScanContext";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {/* Outermost safety net: catches crashes even in context providers,
        so a bad provider state can never produce a blank white screen. */}
    <ErrorBoundary>
      <LanguageProvider>
        <ScanProvider>
          <NotificationCenterProvider>
            <HashRouter>
              <NetworkConnectivityProvider>
                <App />
              </NetworkConnectivityProvider>
            </HashRouter>
          </NotificationCenterProvider>
        </ScanProvider>
      </LanguageProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);

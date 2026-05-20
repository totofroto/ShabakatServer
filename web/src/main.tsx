import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import App from "./App";
import { LanguageProvider } from "./context/LanguageContext";
import { NotificationCenterProvider } from "./context/NotificationCenterContext";
import { NetworkConnectivityProvider } from "./context/NetworkConnectivityContext";
import { ScanProvider } from "./context/ScanContext";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
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
  </React.StrictMode>,
);

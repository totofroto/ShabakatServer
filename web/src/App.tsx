import { useEffect, useRef } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { isTauri } from "@/lib/transport";
import {
  attachConsole,
  debug,
  error as logError,
  info,
  trace,
  warn,
} from "@tauri-apps/plugin-log";
import {
  checkPermissions,
  requestPermissions,
} from "@tauri-apps/plugin-geolocation";
import { IntruderAlertListener } from "@/components/intruder-alert-listener";
import { AppShell } from "@/components/app-shell";
import { ActivityPage } from "@/pages/activity-page";
import { AlertsPage } from "@/pages/alerts-page";
import { DashboardPage } from "@/pages/dashboard-page";
import { DevicesPage } from "@/pages/devices-page";
import { SettingsPage } from "@/pages/settings-page";
import { ToolsPage } from "@/pages/tools-page";
import { LoginPage } from "@/pages/login-page";
import { AuthProvider } from "@/context/AuthContext";

function AppContent() {
  const logDetachRef = useRef<(() => void) | undefined>(undefined);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    // Rust + web `console` → physical log (see `shabakat-blackbox.log` in the app log dir).
    type ConsoleMethod = "log" | "debug" | "info" | "warn" | "error";
    const originals: Record<ConsoleMethod, typeof console.log> = {
      log: console.log.bind(console),
      debug: console.debug.bind(console),
      info: console.info.bind(console),
      warn: console.warn.bind(console),
      error: console.error.bind(console),
    };
    const forward = (
      name: ConsoleMethod,
      toPlugin: (message: string) => Promise<void>,
    ) => {
      console[name] = (...args: unknown[]) => {
        originals[name](...args);
        const message = args
          .map((a) => {
            if (a instanceof Error) {
              return a.stack ?? a.message;
            }
            if (typeof a === "string") {
              return a;
            }
            try {
              return JSON.stringify(a);
            } catch {
              return String(a);
            }
          })
          .join(" ");
        void toPlugin(message);
      };
    };
    void (async () => {
      const detach = await attachConsole();
      logDetachRef.current = detach;
      forward("log", trace);
      forward("debug", debug);
      forward("info", info);
      forward("warn", warn);
      forward("error", logError);
    })();
    return () => {
      (["log", "debug", "info", "warn", "error"] as const).forEach((k) => {
        console[k] = originals[k] as (
          message?: unknown,
          ...p: unknown[]
        ) => void;
      });
      logDetachRef.current?.();
      logDetachRef.current = undefined;
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    void (async () => {
      try {
        await requestPermissions(["location"]);
      } catch (err) {
        console.error("Initial location permission request failed:", err);
      }
      try {
        await checkPermissions();
      } catch {
        /* optional follow-up read */
      }
    })();
  }, []);

  return (
    <>
      <IntruderAlertListener />
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/" element={<AppShell />}>
          <Route index element={<DashboardPage />} />
          <Route path="devices" element={<DevicesPage />} />
          <Route path="activity" element={<ActivityPage />} />
          <Route path="alerts" element={<AlertsPage />} />
          <Route
            path="settings"
            element={<SettingsPage />}
          />
          <Route path="tools" element={<ToolsPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </>
  );
}

function App() {
  return (
    <AuthProvider>
      <AppContent />
    </AuthProvider>
  );
}

export default App;

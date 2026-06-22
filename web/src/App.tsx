import { Navigate, Route, Routes } from "react-router-dom";
import { IntruderAlertListener } from "@/components/intruder-alert-listener";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { AppShell } from "@/components/app-shell";
import { ActivityPage } from "@/pages/activity-page";
import { AlertsPage } from "@/pages/alerts-page";
import { DashboardPage } from "@/pages/dashboard-page";
import { DevicesPage } from "@/pages/devices-page";
import { SettingsPage } from "@/pages/settings-page";
import { ToolsPage } from "@/pages/tools-page";

import { AuthProvider } from "@/context/AuthContext";

function AppContent() {
  return (
    <>
      <IntruderAlertListener />
      {/* Route-level safety net: a crash inside any single page (e.g. the
          D3 topology map hitting a malformed device) shows a Retry card
          instead of taking down the whole shell. */}
      <ErrorBoundary>
        <Routes>

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
      </ErrorBoundary>
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

import { createContext, ReactNode, useCallback, useContext, useEffect, useState } from "react";
import type { DeviceRow, LastScanTelemetry, ScanMode } from "@/hooks/useNetworkScan";
import { useNetworkScan } from "@/hooks/useNetworkScan";
import { invoke } from "@/lib/transport";

type NetworkScoreBreakdown = {
  performance: number;
  latency: number;
  security: number;
};

type SystemStatusResponse = {
  score: number;
  performanceScore: number;
  latencyScore: number;
  securityScore: number;
  lastUpdated: number;
};

type ScanContextValue = {
  devices: DeviceRow[];
  isScanning: boolean;
  isLoading: boolean;
  progressPct: number;
  /** Mean ICMP latency from the last completed LAN scan (ms). */
  averageLatencyMs: number | null;
  scannedHosts: number;
  totalHosts: number;
  triggerScan: (mode?: ScanMode) => Promise<void>;
  cancelScan: () => Promise<void>;
  /** Request location for Android / Netlink; call from Radar before `triggerScan`. */
  ensurePermissions: () => Promise<boolean>;
  scanPermissionError: string | null;
  scanRuntimeError: string | null;
  clearScanPermissionError: () => void;
  clearScanRuntimeError: () => void;
  lastScanAt: Date | null;
  lastSpeedMbps: number | null;
  lastSpeedTestAt: Date | null;
  lastLatencyMs: number | null;
  networkScore: number;
  hasNetworkScoreData: boolean;
  networkScoreBreakdown: NetworkScoreBreakdown;
  recordSpeedTestResult: (downloadMbps: number, latencyMs?: number | null) => void;
  /** Surgically update a single device in state (e.g. after a port scan). */
  patchDevice: (ip: string, patch: Partial<DeviceRow>) => void;
  /** IPC stats from the most recently completed scan. Null before first scan. */
  lastScanTelemetry: LastScanTelemetry | null;
};

const ScanContext = createContext<ScanContextValue | undefined>(undefined);
const SCORE_STORAGE_KEY = "shabakat_network_score_state";

export function ScanProvider({ children }: { children: ReactNode }) {
  const {
    devices,
    isScanning,
    isLoading,
    progressPct,
    averageLatencyMs,
    scannedHosts,
    totalHosts,
    lastScanAt,
    lastScanTelemetry,
    triggerScan,
    cancelScan,
    patchDevice,
    ensurePermissions,
    scanPermissionError,
    scanRuntimeError,
    clearScanPermissionError,
    clearScanRuntimeError,
  } = useNetworkScan();
  const [lastSpeedMbps, setLastSpeedMbps] = useState<number | null>(() => {
    if (typeof window === "undefined") {
      return null;
    }
    try {
      const raw = window.localStorage.getItem(SCORE_STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : null;
      return typeof parsed?.lastSpeedMbps === "number" ? parsed.lastSpeedMbps : null;
    } catch {
      return null;
    }
  });
  const [lastSpeedTestAt, setLastSpeedTestAt] = useState<Date | null>(() => {
    if (typeof window === "undefined") {
      return null;
    }
    try {
      const raw = window.localStorage.getItem(SCORE_STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : null;
      return parsed?.lastSpeedTestAt ? new Date(parsed.lastSpeedTestAt) : null;
    } catch {
      return null;
    }
  });
  const [lastLatencyMs, setLastLatencyMs] = useState<number | null>(() => {
    if (typeof window === "undefined") {
      return null;
    }
    try {
      const raw = window.localStorage.getItem(SCORE_STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : null;
      return typeof parsed?.lastLatencyMs === "number" ? parsed.lastLatencyMs : null;
    } catch {
      return null;
    }
  });
  const [networkScore, setNetworkScore] = useState<number>(() => {
    if (typeof window === "undefined") {
      return 0;
    }
    try {
      const raw = window.localStorage.getItem(SCORE_STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : null;
      return typeof parsed?.networkScore === "number" ? parsed.networkScore : 0;
    } catch {
      return 0;
    }
  });
  const [networkScoreBreakdown, setNetworkScoreBreakdown] =
    useState<NetworkScoreBreakdown>(() => {
      if (typeof window === "undefined") {
        return { performance: 0, latency: 0, security: 30 };
      }
      try {
        const raw = window.localStorage.getItem(SCORE_STORAGE_KEY);
        const parsed = raw ? JSON.parse(raw) : null;
        if (
          typeof parsed?.networkScoreBreakdown?.performance === "number" &&
          typeof parsed?.networkScoreBreakdown?.latency === "number" &&
          typeof parsed?.networkScoreBreakdown?.security === "number"
        ) {
          return parsed.networkScoreBreakdown;
        }
      } catch {
        // ignore malformed local storage
      }
      return { performance: 0, latency: 0, security: 30 };
    });
  const [hasNetworkScoreData, setHasNetworkScoreData] = useState<boolean>(() => {
    if (typeof window === "undefined") {
      return false;
    }
    try {
      const raw = window.localStorage.getItem(SCORE_STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : null;
      return Boolean(parsed?.hasNetworkScoreData);
    } catch {
      return false;
    }
  });

  const fetchNetworkScore = useCallback(async () => {
    try {
      const data = await invoke<SystemStatusResponse>("get_system_status");
      if (data && typeof data.score === "number") {
        setNetworkScore(data.score);
        setNetworkScoreBreakdown({
          performance: data.performanceScore,
          latency: data.latencyScore,
          security: data.securityScore,
        });
        setHasNetworkScoreData(data.lastUpdated > 0);
      }
    } catch (err) {
      console.error("ScanContext: fetchNetworkScore failed", err);
    }
  }, []);

  const recordSpeedTestResult = (downloadMbps: number, latencyMs?: number | null) => {
    setLastSpeedMbps(downloadMbps);
    setLastSpeedTestAt(new Date());
    if (typeof latencyMs === "number" && latencyMs > 0) {
      setLastLatencyMs(latencyMs);
    }
    // Refresh score after a speed test
    void fetchNetworkScore();
  };

  useEffect(() => {
    if (averageLatencyMs !== null && Number.isFinite(averageLatencyMs)) {
      setLastLatencyMs(averageLatencyMs);
    }
  }, [averageLatencyMs]);

  useEffect(() => {
    // Initial fetch
    fetchNetworkScore();
    // Poll every 60 seconds
    const tid = setInterval(fetchNetworkScore, 60_000);
    return () => clearInterval(tid);
  }, [fetchNetworkScore]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(
      SCORE_STORAGE_KEY,
      JSON.stringify({
        lastSpeedMbps,
        lastSpeedTestAt: lastSpeedTestAt?.toISOString() ?? null,
        lastLatencyMs,
        networkScore,
        networkScoreBreakdown,
        hasNetworkScoreData,
      }),
    );
  }, [
    hasNetworkScoreData,
    lastLatencyMs,
    lastSpeedMbps,
    lastSpeedTestAt,
    networkScore,
    networkScoreBreakdown,
  ]);

  return (
    <ScanContext.Provider
      value={{
        devices,
        isScanning,
        isLoading,
        progressPct,
        averageLatencyMs,
        scannedHosts,
        totalHosts,
        triggerScan,
        cancelScan,
        ensurePermissions,
        scanPermissionError,
        scanRuntimeError,
        clearScanPermissionError,
        clearScanRuntimeError,
        lastScanAt,
        lastSpeedMbps,
        lastSpeedTestAt,
        lastLatencyMs,
        networkScore,
        hasNetworkScoreData,
        networkScoreBreakdown,
        recordSpeedTestResult,
        patchDevice,
        lastScanTelemetry,
      }}
    >
      {children}
    </ScanContext.Provider>
  );
}

export function useScanContext() {
  const context = useContext(ScanContext);
  if (!context) {
    throw new Error("useScanContext must be used within a ScanProvider");
  }
  return context;
}

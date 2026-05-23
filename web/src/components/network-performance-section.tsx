import { invoke, isTauri, subscribeTelemetryEvents, type SystemTelemetry } from "@/lib/transport";
import { listen } from "@/lib/transport";
import {
  Activity,
  ArrowDownUp,
  Gauge,
  Globe2,
  Loader2,
  RadioTower,
  Router,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useScanContext } from "@/context/ScanContext";
import { useLanguage } from "@/context/LanguageContext";
import { cn } from "@/lib/utils";

const UI = {
  sectionTitle:       { en: "Network Performance",           ar: "أداء الشبكة" },
  sectionDesc:        { en: "Dual-engine view: live LAN link traffic and a Cloudflare-backed WAN speed test.", ar: "عرض مزدوج: حركة شبكة LAN المباشرة واختبار سرعة WAN عبر Cloudflare." },
  lanLabel:           { en: "Local link (LAN)",               ar: "الرابط المحلي (LAN)" },
  lanTitle:           { en: "Active interface",               ar: "الواجهة النشطة" },
  liveDownload:       { en: "Live Download",                  ar: "التنزيل المباشر" },
  liveUpload:         { en: "Live Upload",                    ar: "الرفع المباشر" },
  loadingLink:        { en: "Loading link stats…",            ar: "جارٍ تحميل إحصائيات الرابط…" },
  tauriRequired:      { en: "LAN stats require the Tauri shell.", ar: "تتطلب إحصائيات LAN قشرة Tauri." },
  wanLabel:           { en: "Internet speed (WAN)",           ar: "سرعة الإنترنت (WAN)" },
  wanTitle:           { en: "Cloudflare throughput",          ar: "إنتاجية Cloudflare" },
  downloadLive:       { en: "Download (live)",                ar: "التنزيل (مباشر)" },
  runTest:            { en: "Run speed test",                 ar: "اختبار السرعة" },
  runningTest:        { en: "Running test…",                  ar: "جارٍ الاختبار…" },
  ping:               { en: "Ping",                           ar: "البينغ" },
  download:           { en: "Download",                       ar: "تنزيل" },
  upload:             { en: "Upload",                         ar: "رفع" },
  runTestHint:        { en: "Run a test to measure ping (to 1.1.1.1), download (10 MB), and upload (1 MB POST).", ar: "شغّل اختباراً لقياس البينغ (إلى 1.1.1.1)، والتنزيل (10 ميغابايت)، والرفع (POST بحجم 1 ميغابايت)." },
  mbpsDown:           { en: "Mbps down",                           ar: "ميغابت/ث تنزيل" },
} as const;

type LinkStatsPayload = {
  interfaceName: string;
  connectionType: string;
  liveRxBytes: number;
  liveTxBytes: number;
};

type SpeedTestPayload = {
  pingMs: number;
  downloadMbps: number;
  uploadMbps: number;
};

type SpeedTestProgressPayload = {
  downloadMbps: number;
  bytesDownloaded: number;
  progressPct: number | null;
};

function formatSpeedTestError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Speed test failed";
}

function formatBitrate(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec < 0) {
    return "—";
  }
  if (bytesPerSec < 1024) {
    return `${Math.round(bytesPerSec)} B/s`;
  }
  const k = bytesPerSec / 1024;
  if (k < 1024) {
    return `${k >= 10 ? k.toFixed(0) : k.toFixed(1)} KB/s`;
  }
  const m = k / 1024;
  return `${m >= 10 ? m.toFixed(1) : m.toFixed(2)} MB/s`;
}

function SpeedGaugeMini({
  currentMbps,
  progressPct,
  isActive,
  label,
}: {
  currentMbps: number;
  progressPct: number;
  isActive: boolean;
  label: string;
}) {
  const width = 240;
  const height = 140;
  const radius = 100;
  const centerX = width / 2;
  const centerY = 130;
  const stroke = 12;
  const progress = Math.min(Math.max(progressPct, 0), 100);
  const needleAngle = -90 + (progress / 100) * 180;
  const radians = (needleAngle * Math.PI) / 180;
  const needleLength = radius - 18;
  const needleX = centerX + needleLength * Math.cos(radians);
  const needleY = centerY + needleLength * Math.sin(radians);

  return (
    <div
      className={cn(
        "relative mx-auto flex w-full max-w-[240px] items-center justify-center overflow-visible",
        isActive && "animate-pulse",
      )}
    >
      <svg
        viewBox={`0 0 ${width} ${height}`}
        className="h-32 w-full"
        preserveAspectRatio="xMidYMid meet"
        aria-hidden
      >
        <path
          d={`M20 ${centerY} A${radius} ${radius} 0 0 1 ${width - 20} ${centerY}`}
          fill="none"
          stroke="var(--chart-grid)"
          strokeWidth={stroke}
          strokeLinecap="round"
        />
        <path
          d={`M20 ${centerY} A${radius} ${radius} 0 0 1 ${width - 20} ${centerY}`}
          fill="none"
          stroke="var(--accent)"
          strokeWidth={stroke}
          strokeLinecap="round"
          pathLength={100}
          strokeDasharray={`${progress} 100`}
          className="transition-all duration-200 ease-out"
        />
        <line
          x1={centerX}
          y1={centerY}
          x2={needleX}
          y2={needleY}
          stroke="var(--text-primary)"
          strokeWidth={2.5}
          strokeLinecap="round"
        />
        <circle cx={centerX} cy={centerY} r={6} fill="var(--text-primary)" />
      </svg>
      <div className="absolute inset-x-0 bottom-0 flex flex-col items-center pb-0.5 text-center">
        <span className="text-2xl font-semibold tabular-nums text-primary">
          {currentMbps.toFixed(1)}
        </span>
        <span className="text-[10px] uppercase tracking-wider text-secondary">
          {label}
        </span>
      </div>
    </div>
  );
}

export function NetworkPerformanceSection() {
  const { lang } = useLanguage();
  const t = (key: keyof typeof UI) => (UI[key] as any)[lang] || UI[key].en;
  const { recordSpeedTestResult } = useScanContext();
  const [link, setLink] = useState<LinkStatsPayload | null>(null);
  const [linkErr, setLinkErr] = useState<string | null>(null);
  // Computed throughput in bytes/s (delta from consecutive raw counter reads).
  const [rxRate, setRxRate] = useState(0);
  const [txRate, setTxRate] = useState(0);
  const prevLinkRef = useRef<{ rx: number; tx: number; ts: number } | null>(null);
  const [isTesting, setIsTesting] = useState(false);
  const [wanError, setWanError] = useState<string | null>(null);
  const [wanResult, setWanResult] = useState<SpeedTestPayload | null>(null);
  const [gaugeMbps, setGaugeMbps] = useState(0);
  const [gaugePct, setGaugePct] = useState(0);

  const pollLink = useCallback(async () => {
    if (!isTauri()) {
      return;
    }
    try {
      console.log("[JS_TRACE] Polling LAN Stats...");
      const s = await invoke<LinkStatsPayload>("get_local_link_stats");
      console.log("[JS_TRACE] LAN Stats Result:", s);
      const now = Date.now();
      const prev = prevLinkRef.current;
      if (prev) {
        const dt = (now - prev.ts) / 1000; // seconds
        if (dt > 0.05) {
          setRxRate(Math.max(0, (s.liveRxBytes - prev.rx) / dt));
          setTxRate(Math.max(0, (s.liveTxBytes - prev.tx) / dt));
        }
      }
      prevLinkRef.current = { rx: s.liveRxBytes, tx: s.liveTxBytes, ts: now };
      setLink(s);
      setLinkErr(null);
    } catch (e) {
      setLinkErr(formatSpeedTestError(e));
    }
  }, []);

  // 1. LAN Stats: Polling (Tauri) or WebSocket (Standalone)
  useEffect(() => {
    if (isTauri()) {
      void pollLink();
      const id = window.setInterval(() => void pollLink(), 1000);
      return () => window.clearInterval(id);
    } else {
      // Standalone mode: subscribe to live telemetry stream
      console.log("[TRANSPORT] Subscribing to live telemetry stream...");
      const unsubscribe = subscribeTelemetryEvents((event, data) => {
        if (event === "system_telemetry") {
          const telemetry = data as SystemTelemetry;
          // Find the primary interface (excluding loopback)
          const primary = telemetry.interfaces.find(i => i.interface !== "lo" && (i.bytesRxPerSec > 0 || i.bytesTxPerSec > 0)) 
                          || telemetry.interfaces[0];
          
          if (primary) {
            setRxRate(primary.bytesRxPerSec);
            setTxRate(primary.bytesTxPerSec);
            setLink({
              interfaceName: primary.interface,
              connectionType: "LAN",
              liveRxBytes: 0, // Not used when rate is provided directly
              liveTxBytes: 0,
            });
          }
        }
      });
      return unsubscribe;
    }
  }, [pollLink]);

  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | null = null;
    const setup = async () => {
      unlisten = await listen<SpeedTestProgressPayload>("speed-test-progress", (e) => {
        if (!isMounted) {
          return;
        }
        setGaugeMbps(e.payload.downloadMbps ?? 0);
        if (typeof e.payload.progressPct === "number") {
          setGaugePct(e.payload.progressPct);
        }
      });
    };
    void setup();
    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, []);

  const runWan = async () => {
    setIsTesting(true);
    setWanError(null);
    setWanResult(null);
    setGaugeMbps(0);
    setGaugePct(0);
    try {
      const result = await invoke<SpeedTestPayload>("run_speed_test");
      setWanResult(result);
      setGaugeMbps(result.downloadMbps);
      setGaugePct(100);
      recordSpeedTestResult(result.downloadMbps, result.pingMs);
    } catch (e) {
      setWanError(formatSpeedTestError(e));
    } finally {
      setIsTesting(false);
    }
  };

  return (
    <section className="space-y-3">
      <div className="px-0.5">
        <h2 className="text-lg font-semibold tracking-tight text-primary">
          {t("sectionTitle")}
        </h2>
        <p className="text-sm text-secondary">{t("sectionDesc")}</p>
      </div>
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {/* PANEL A — LAN */}
        <Card className="relative overflow-hidden">
          <CardHeader className="pb-2">
            <div className="flex items-center gap-2 text-secondary">
              <Router className="size-5" aria-hidden />
              <CardDescription className="text-[11px] font-semibold uppercase tracking-[0.2em] text-secondary">
                {t("lanLabel")}
              </CardDescription>
            </div>
            <CardTitle className="text-base text-primary">
              {t("lanTitle")}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {linkErr ? (
              <p className="text-sm text-error">{linkErr}</p>
            ) : link ? (
              <>
                <div className="flex flex-wrap items-center gap-2">
                  <span
                    className={cn(
                      "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold",
                      "border-online/25 bg-online/10 text-online",
                    )}
                  >
                    <Activity
                      className="size-3.5 animate-pulse"
                      aria-hidden
                    />
                    {link.connectionType}
                  </span>
                  <code className="rounded-md bg-surface-alt px-2 py-0.5 font-mono text-xs text-secondary">
                    {link.interfaceName}
                  </code>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="rounded-xl bg-surface-alt p-3">
                    <div className="mb-1 flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wider text-tertiary">
                      <ArrowDownUp className="size-3" aria-hidden />
                      {t("liveDownload")}
                    </div>
                    <p
                      className={cn(
                        "text-[15px] font-medium tabular-nums text-primary",
                        rxRate > 0 && "pulse-live",
                      )}
                    >
                      {formatBitrate(rxRate)}
                    </p>
                  </div>
                  <div className="rounded-xl bg-surface-alt p-3">
                    <div className="mb-1 flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wider text-tertiary">
                      <ArrowDownUp
                        className="size-3 rotate-180"
                        aria-hidden
                      />
                      {t("liveUpload")}
                    </div>
                    <p
                      className={cn(
                        "text-[15px] font-medium tabular-nums text-primary",
                        txRate > 0 && "pulse-live",
                      )}
                    >
                      {formatBitrate(txRate)}
                    </p>
                  </div>
                </div>
              </>
            ) : (
              <p className="text-sm text-secondary">{t("loadingLink")}</p>
            )}
          </CardContent>
        </Card>

        {/* PANEL B — WAN */}
        <Card className="relative overflow-hidden">
          <CardHeader className="pb-2">
            <div className="flex items-center gap-2 text-secondary">
              <Globe2 className="size-5" aria-hidden />
              <CardDescription className="text-[11px] font-semibold uppercase tracking-[0.2em] text-secondary">
                {t("wanLabel")}
              </CardDescription>
            </div>
            <CardTitle className="text-base text-primary">
              {t("wanTitle")}
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex flex-col items-stretch gap-3 sm:flex-row sm:items-start">
              <div className="min-w-0 flex-1">
                <div className="mb-1 flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wider text-tertiary">
                  <Gauge className="size-3" aria-hidden />
                  {t("downloadLive")}
                </div>
                <SpeedGaugeMini
                  currentMbps={gaugeMbps}
                  progressPct={isTesting ? gaugePct : Math.min((gaugeMbps / 500) * 100, 100)}
                  isActive={isTesting}
                  label={t("mbpsDown")}
                />
              </div>
              <div className="flex w-full flex-col gap-2 sm:w-auto sm:pt-6">
                <Button
                  type="button"
                  onClick={runWan}
                  disabled={isTesting}
                  className="w-full gap-2 sm:w-auto"
                >
                  {isTesting ? (
                    <>
                      <Loader2
                        className="size-4 shrink-0 animate-spin"
                        aria-hidden
                      />
                      {t("runningTest")}
                    </>
                  ) : (
                    <>
                      <RadioTower className="size-4 shrink-0" aria-hidden />
                      {t("runTest")}
                    </>
                  )}
                </Button>
                {wanError ? (
                  <p className="text-sm text-error">{wanError}</p>
                ) : null}
              </div>
            </div>
            {wanResult ? (
              <div className="grid grid-cols-1 gap-2 rounded-xl bg-surface-alt p-3 sm:grid-cols-3">
                <div>
                  <p className="text-[10px] font-bold uppercase tracking-wider text-tertiary">
                    {t("ping")}
                  </p>
                  <p className="text-xl font-bold tabular-nums text-primary">
                    {wanResult.pingMs < 0.1
                      ? "—"
                      : `${Math.round(wanResult.pingMs)} ms`}
                  </p>
                </div>
                <div>
                  <p className="text-[10px] font-bold uppercase tracking-wider text-tertiary">
                    {t("download")}
                  </p>
                  <p className="text-xl font-bold tabular-nums text-online">
                    {wanResult.downloadMbps.toFixed(1)} Mbps
                  </p>
                </div>
                <div>
                  <p className="text-[10px] font-bold uppercase tracking-wider text-tertiary">
                    {t("upload")}
                  </p>
                  <p className="text-xl font-bold tabular-nums text-accent">
                    {wanResult.uploadMbps.toFixed(1)} Mbps
                  </p>
                </div>
              </div>
            ) : (
              <p className="text-xs text-secondary">{t("runTestHint")}</p>
            )}
          </CardContent>
        </Card>
      </div>
    </section>
  );
}

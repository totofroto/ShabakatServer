import { Loader2, RadioTower, Signal, Wifi } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { invoke, isTauri } from "@/lib/transport";
import { NetworkPerformanceSection } from "@/components/network-performance-section";
import { TopologyMap } from "@/components/topology-map";
import { cn } from "@/lib/utils";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { useLanguage } from "@/context/LanguageContext";
import { useNetworkConnectivity } from "@/context/NetworkConnectivityContext";
import { useScanContext } from "@/context/ScanContext";

const RING_RADIUS = 88;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

type WifiInfoPayload = {
  ssid: string | null;
  frequencyBand: string;
  wifiStandard: string | null;
};

type Outage = {
  id: number;
  startedAt: number;
  endedAt: number | null;
  durationMs: number | null;
  ongoing: boolean;
};

type SpeedResult = {
  downloadMbps: number | null;
  uploadMbps: number | null;
  pingMs: number | null;
  testedAt: number;
};

const UI = {
  pillNotMeasured:     { en: "Network: Not measured",          ar: "الشبكة: غير مقيَّمة" },
  pillSecure:          { en: "Network: Secure & optimal",      ar: "الشبكة: آمنة ومثلى" },
  pillStable:          { en: "Network: Stable",                ar: "الشبكة: مستقرة" },
  pillAttention:       { en: "Network: Needs attention",       ar: "الشبكة: تحتاج اهتماماً" },
  posture:             { en: "Posture",                        ar: "الوضع" },
  notMeasured:         { en: "Not Measured",                   ar: "غير مقيَّم" },
  excellent:           { en: "Excellent",                      ar: "ممتاز" },
  good:                { en: "Good",                           ar: "جيد" },
  needsAttention:      { en: "Needs Attention",                ar: "يحتاج اهتماماً" },
  healthShieldLabel:   { en: "Network Health Shield",          ar: "درع صحة الشبكة" },
  healthShieldTitle:   { en: "Overall Security & Performance", ar: "الأمان والأداء العام" },
  shieldHint:          { en: "Click the shield for score details.", ar: "اضغط على الدرع لرؤية تفاصيل الدرجة." },
  networkStatus:       { en: "Network Status",                 ar: "حالة الشبكة" },
  localNetwork:        { en: "Local Network",                  ar: "الشبكة المحلية" },
  offline:             { en: "Offline",                        ar: "غير متصل" },
  online:              { en: "Online",                         ar: "متصل" },
  unknown:             { en: "Unknown",                        ar: "غير معروف" },
  devicesLabel:        { en: "Devices",                        ar: "الأجهزة" },
  onlineCount:         { en: "{n} currently reporting online", ar: "{n} متصل حالياً" },
  lastSpeedTest:       { en: "Last Speed Test",                ar: "آخر اختبار سرعة" },
  lastTestedAt:        { en: "Last tested at {time}",          ar: "آخر اختبار في {time}" },
  noSpeedTestYet:      { en: "No speed test run yet",          ar: "لم يُجرَ اختبار سرعة بعد" },
  networkOverview:     { en: "Network Overview",               ar: "نظرة عامة على الشبكة" },
  networkOverviewDesc: { en: "Real-time NOC view of your local network posture.", ar: "عرض مركز العمليات في الوقت الفعلي لوضع شبكتك المحلية." },
  recentActivity:      { en: "Recent Activity",                ar: "النشاط الأخير" },
  recentActivityDesc:  { en: "Last 5 online devices detected in the most recent scan.", ar: "آخر 5 أجهزة متصلة اكتُشفت في آخر فحص." },
  noRecentDevices:     { en: "No recent online devices yet. Run a network scan from Devices.", ar: "لا توجد أجهزة متصلة حديثاً. شغّل فحص الشبكة من صفحة الأجهزة." },
  onlineWithVendor:    { en: "Online · {vendor}",              ar: "متصل · {vendor}" },
  scoreBreakdown:      { en: "Score Breakdown",                ar: "تفصيل الدرجة" },
  speed:               { en: "Speed:",                         ar: "السرعة:" },
  latency:             { en: "Latency:",                       ar: "زمن الاستجابة:" },
  security:            { en: "Security:",                      ar: "الأمان:" },
  notMeasuredShort:    { en: "not measured",                   ar: "غير مقيَّس" },
  deductsNote:         { en: "Deducts 5 points per unknown vendor or randomized MAC found in the latest scan.", ar: "يُخصم 5 نقاط لكل بائع مجهول أو MAC عشوائي في آخر فحص." },
  internetHealth:      { en: "Internet Health",                ar: "صحة الإنترنت" },
  internetOnline:      { en: "✅ Online",                      ar: "✅ متصل" },
  internetDown:        { en: "🔴 Down since {time}",           ar: "🔴 منقطع منذ {time}" },
  lastOutage:          { en: "Last outage: {min} min, {days} days ago", ar: "آخر انقطاع: {min} د، منذ {days} أيام" },
  noOutages:           { en: "No outages recorded",            ar: "لا انقطاعات مسجَّلة" },
  runSpeedTest:        { en: "Run Speed Test",                 ar: "تشغيل اختبار السرعة" },
  testing:             { en: "Testing…",                       ar: "جارٍ الاختبار…" },
  speedHistory:        { en: "Recent results",                 ar: "نتائج سابقة" },
  // Browser-mode additions
  waddanTitle:         { en: "WADDAN — Home Network",         ar: "شبكة المنزل — WADDAN" },
  waddanSubtitle:      { en: "Live monitoring",               ar: "مراقبة مباشرة" },
  knownLabel:          { en: "known",                         ar: "معروف" },
  uptimeLabel:         { en: "Uptime",                        ar: "وقت التشغيل" },
  noOutagesToday:      { en: "No outages today",              ar: "لا انقطاعات اليوم" },
  outageHistoryTitle:  { en: "Outage History (7d)",           ar: "سجل الانقطاعات (7 أيام)" },
  noOutagePast7d:      { en: "No outages in the last 7 days ✅", ar: "لا انقطاعات خلال 7 أيام ✅" },
  ongoing:             { en: "Ongoing",                       ar: "جارٍ" },
  speedTestTitle:      { en: "Speed Test",                    ar: "اختبار السرعة" },
  speedHistoryTable:   { en: "History",                       ar: "السجل" },
  noHistoryYet:        { en: "No results yet — run a test.",  ar: "لا نتائج بعد — شغّل اختباراً." },
  pingLabel:           { en: "Ping",                          ar: "بينغ" },
  launchPortal:        { en: "Launch Portal",                 ar: "فتح البوابة" },
} as const;

// ── Pure helpers ──────────────────────────────────────────────────────────────

function fmtTime(tsMs: number): string {
  return new Date(tsMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function fmtMins(ms: number | null): number {
  if (ms === null) return 0;
  return Math.round(ms / 60_000);
}

function daysAgo(tsMs: number): number {
  return Math.floor((Date.now() - tsMs) / 86_400_000);
}

function timeAgoStr(date: Date | null | undefined): string {
  if (!date) return "—";
  const secs = Math.floor((Date.now() - date.getTime()) / 1000);
  if (secs < 90) return "just now";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  return `${Math.floor(mins / 60)}h ago`;
}

function gatewayToSubnet(gateway: string | null | undefined): string {
  if (!gateway) return "";
  const parts = gateway.split(".");
  if (parts.length !== 4) return "";
  return `${parts[0]}.${parts[1]}.${parts[2]}.0/24`;
}

// ── Sub-components ────────────────────────────────────────────────────────────

function NetworkHealthDisplay({
  score,
  hasNetworkScoreData,
}: {
  score: number;
  hasNetworkScoreData: boolean;
}) {
  const { lang } = useLanguage();
  const t = (key: keyof typeof UI) => (UI[key] as any)[lang] || UI[key].en;

  const displayScore = hasNetworkScoreData ? Math.min(Math.max(score, 0), 100) : 0;
  const dashOffset =
    RING_CIRCUMFERENCE - (RING_CIRCUMFERENCE * displayScore) / 100;

  const ringClass =
    !hasNetworkScoreData
      ? "text-tertiary"
      : score >= 90
        ? "text-online"
        : score >= 70
          ? "text-warning"
          : "text-error";

  const pillContent = !hasNetworkScoreData
    ? t("pillNotMeasured")
    : score >= 90
      ? t("pillSecure")
      : score >= 70
        ? t("pillStable")
        : t("pillAttention");

  const pillBorder =
    !hasNetworkScoreData
      ? "border-separator bg-surface-alt text-secondary"
      : score >= 90
        ? "border-online/20 bg-online/10 text-online"
        : score >= 70
          ? "border-warning/20 bg-warning/10 text-warning"
          : "border-error/20 bg-error/10 text-error";

  return (
    <section className="flex flex-col items-center justify-center py-6 md:py-10">
      <div className="mx-auto flex w-full justify-center">
        <div className="group relative flex h-48 w-48 shrink-0 items-center justify-center">
        <svg
          className="absolute inset-0 size-full -rotate-90"
          viewBox="0 0 192 192"
          aria-hidden
        >
          <circle
            cx="96"
            cy="96"
            r={RING_RADIUS}
            stroke="var(--chart-grid)"
            strokeWidth="8"
            fill="transparent"
          />
          <circle
            cx="96"
            cy="96"
            r={RING_RADIUS}
            stroke="currentColor"
            strokeWidth="8"
            fill="transparent"
            strokeDasharray={RING_CIRCUMFERENCE}
            strokeDashoffset={dashOffset}
            strokeLinecap="round"
            className={`transition-all duration-1000 ${ringClass}`}
          />
        </svg>

        <div className="z-10 text-center">
          <span className="text-6xl font-black text-primary tabular-nums">
            {hasNetworkScoreData ? score : "--"}
          </span>
          <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-accent">
            {t("posture")}
          </p>
        </div>
        </div>
      </div>
      <div
        className={`mt-6 rounded-full border px-4 py-1.5 text-xs font-bold uppercase tracking-wider ${pillBorder}`}
      >
        {pillContent}
      </div>
    </section>
  );
}

/** Compact NOC status bar — browser mode only. Bleeds to main's edge via -mx-8 -mt-8. */
function NocStatusBar({
  nowStr,
  internetOnline,
  onlineCount,
  totalCount,
  lastScanAgo,
}: {
  nowStr: string;
  internetOnline: boolean;
  onlineCount: number;
  totalCount: number;
  lastScanAgo: string;
}) {
  return (
    <div className="-mx-8 -mt-8 mb-6 flex h-10 shrink-0 items-center justify-between border-b border-separator/80 bg-void px-6">
      <div className="flex items-center gap-4">
        <span className="font-mono text-[10px] font-bold tracking-widest text-secondary tabular-nums">{nowStr}</span>
        <div className="h-3 w-px bg-separator/50" />
        <span className="font-mono text-[10px] font-bold tracking-widest text-accent uppercase">System: Operational</span>
      </div>
      <div className="flex items-center gap-6 text-[10px] font-bold uppercase tracking-widest">
        <span className="flex items-center gap-2">
          <span
            className={cn("h-1.5 w-1.5 rounded-full", internetOnline ? "bg-online shadow-[0_0_8px_rgba(48,209,88,0.6)]" : "bg-error animate-pulse")}
            aria-hidden
          />
          <span className={internetOnline ? "text-online" : "text-error"}>
            WAN: {internetOnline ? "Linked" : "Disconnected"}
          </span>
        </span>
        <span className="text-accent">
          LAN: {onlineCount}/{totalCount} Active
        </span>
        <span className="text-secondary">Scan: {lastScanAgo}</span>
      </div>
      <span className="font-mono text-[10px] font-bold tracking-widest text-tertiary">NODE-01 · 192.168.254.18</span>
    </div>
  );
}

/** Compact stat card for the browser 4-column row. */
function BrowserStatCard({
  label,
  value,
  sub,
  subClass = "text-secondary",
}: {
  label: string;
  value: React.ReactNode;
  sub?: string;
  subClass?: string;
}) {
  return (
    <div className="flex h-24 flex-col justify-between rounded-xl bg-surface/50 p-5 ring-1 ring-inset ring-separator/40 hover:bg-surface-alt/50 transition-colors">
      <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-tertiary">{label}</p>
      <div className="flex items-baseline gap-2">
        <p className="text-3xl font-black leading-none tabular-nums text-primary">{value}</p>
      </div>
      {sub && <p className={cn("truncate text-[10px] font-bold uppercase tracking-wider", subClass)}>{sub}</p>}
    </div>
  );
}

/** Speed test section for browser mode — replaces NetworkPerformanceSection. */
function BrowserSpeedSection({
  speedRunning,
  speedResult,
  speedHistory,
  lastLatencyMs,
  onRunSpeedTest,
}: {
  speedRunning: boolean;
  speedResult: SpeedResult | null;
  speedHistory: SpeedResult[];
  lastLatencyMs: number | null;
  onRunSpeedTest: () => Promise<void>;
}) {
  const { dict } = useLanguage();
  const latestPing = speedResult?.pingMs ?? (speedHistory[0]?.pingMs ?? lastLatencyMs);

  return (
    <Card className="bg-surface/30 border-separator/40">
      <CardHeader className="pb-4">
        <div className="flex items-center justify-between">
          <div>
            <CardDescription className="text-[10px] font-bold uppercase tracking-[0.25em] text-tertiary">
              {dict.linkPerformance}
            </CardDescription>
            {speedResult || speedHistory.length > 0 ? (
              <CardTitle className="mt-1 text-3xl font-black tracking-tight text-accent">
                ↓{(speedResult ?? speedHistory[0]).downloadMbps?.toFixed(1)} <span className="text-lg font-bold text-tertiary">/</span> ↑{(speedResult ?? speedHistory[0]).uploadMbps?.toFixed(1)} <span className="text-xs font-bold uppercase tracking-widest text-secondary">Mbps</span>
              </CardTitle>
            ) : (
              <CardTitle className="mt-1 text-3xl font-black text-secondary">—</CardTitle>
            )}
          </div>
          <Button
            size="sm"
            variant="outline"
            disabled={speedRunning}
            onClick={onRunSpeedTest}
            className="h-8 gap-2 border-accent/30 bg-accent/5 text-[10px] font-bold uppercase tracking-widest text-accent hover:bg-accent/15"
          >
            {speedRunning ? (
              <>
                <Loader2 className="size-3 animate-spin" aria-hidden />
                Executing…
              </>
            ) : (
              <>
                <RadioTower className="size-3" aria-hidden />
                Run Probes
              </>
            )}
          </Button>
        </div>
        {(speedResult ?? speedHistory[0]) && (
          <div className="mt-2 flex items-center gap-4 text-[10px] font-bold uppercase tracking-widest text-secondary">
            <span>Latency: <span className="text-primary">{latestPing != null ? `${latestPing.toFixed(0)}ms` : "—"}</span></span>
            <div className="h-2 w-px bg-separator/50" />
            <span>Last Probe: <span className="text-primary">{fmtTime((speedResult ?? speedHistory[0])!.testedAt)}</span></span>
          </div>
        )}
      </CardHeader>
      <CardContent className="space-y-4">
        {speedHistory.length > 0 && (
          <div className="space-y-2">
            <p className="text-[9px] font-bold uppercase tracking-widest text-tertiary">Historical Snapshot (Recent 5)</p>
            <div className="rounded-xl border border-separator/30 bg-void/50 overflow-hidden">
              <div className="grid grid-cols-4 gap-x-3 px-4 py-2 text-[9px] font-bold uppercase tracking-[0.15em] text-tertiary border-b border-separator/20">
                <span>Timestamp</span>
                <span className="text-right">RX Rate</span>
                <span className="text-right">TX Rate</span>
                <span className="text-right">RTT</span>
              </div>
              {speedHistory.slice(0, 5).map((h) => (
                <div key={h.testedAt} className="group hover:bg-surface-alt/20 transition-colors">
                  <div className="grid grid-cols-4 gap-x-3 px-4 py-2.5 text-[11px] font-mono tracking-tight">
                    <span className="text-secondary">{fmtTime(h.testedAt)}</span>
                    <span className="text-right font-bold text-online">
                      {h.downloadMbps?.toFixed(1) ?? "—"}<span className="ml-1 text-[9px] text-tertiary">Mb</span>
                    </span>
                    <span className="text-right font-bold text-accent">
                      {h.uploadMbps?.toFixed(1) ?? "—"}<span className="ml-1 text-[9px] text-tertiary">Mb</span>
                    </span>
                    <span className="text-right text-secondary">
                      {h.pingMs != null ? `${h.pingMs.toFixed(0)}ms` : "—"}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {speedHistory.length === 0 && !speedRunning && (
          <div className="flex h-24 flex-col items-center justify-center rounded-xl border border-dashed border-separator/40 bg-void/30">
            <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-tertiary">
              No performance metrics recorded
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

/** Compact outage history list — last 7 days, max 5 rows. */
function OutageHistoryCard({ outages }: { outages: Outage[] }) {
  const sevenDaysAgo = Date.now() - 7 * 86_400_000;
  const ongoing = outages.find((o) => o.ongoing) ?? null;
  const recent = outages
    .filter((o) => !o.ongoing && o.startedAt >= sevenDaysAgo)
    .slice(0, 5);

  const hasAny = ongoing !== null || recent.length > 0;

  return (
    <Card className="bg-surface/30 border-separator/40">
      <CardHeader className="pb-2">
        <CardDescription className="text-[10px] font-bold uppercase tracking-[0.25em] text-tertiary">
          Incident Log (7d)
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-1">
        {!hasAny ? (
          <div className="flex items-center gap-2 px-1 py-1 text-[11px] font-bold uppercase tracking-widest text-online">
            <span className="h-1.5 w-1.5 rounded-full bg-online shadow-[0_0_8px_rgba(48,209,88,0.6)]" />
            Zero incidents detected
          </div>
        ) : (
          <div className="space-y-1.5">
            {ongoing && (
              <div className="flex items-center gap-3 rounded-lg border border-error/30 bg-error/5 px-3 py-2 text-[11px] font-mono">
                <span className="h-1.5 w-1.5 rounded-full bg-error animate-pulse shadow-[0_0_8px_rgba(255,69,58,0.6)]" aria-hidden />
                <span className="font-bold text-error">
                  CRITICAL: Ongoing outage since {fmtTime(ongoing.startedAt)}
                </span>
              </div>
            )}
            {recent.map((o) => (
              <div
                key={o.id}
                className="flex items-center gap-3 rounded-lg border border-separator/20 bg-void/50 px-3 py-2 text-[11px] font-mono"
              >
                <span className="h-1 w-1 rounded-full bg-secondary" aria-hidden />
                <span className="text-secondary">
                  RECOVERY: {fmtTime(o.startedAt)} → {o.endedAt ? fmtTime(o.endedAt) : "?"}{" "}
                  <span className="ml-1 text-tertiary">({fmtMins(o.durationMs)}m duration)</span>
                </span>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

type ApiNetwork = { ssid: string | null; gateway: string | null };

export function DashboardPage() {
  const { lang, dict } = useLanguage();
  const t = (key: keyof typeof UI) => (UI[key] as any)[lang] || UI[key].en;
  const [wifiInfo, setWifiInfo] = useState<WifiInfoPayload | null>(null);
  const [apiNetwork, setApiNetwork] = useState<ApiNetwork | null>(null);
  const { networkState, lanScanAllowed } = useNetworkConnectivity();
  const {
    devices,
    isScanning,
    lastSpeedMbps,
    lastSpeedTestAt,
    lastLatencyMs,
    networkScore,
    hasNetworkScoreData,
    networkScoreBreakdown,
    lastScanAt,
  } = useScanContext();

  // Clock — updates every second in browser mode
  const [nowStr, setNowStr] = useState(() =>
    new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }),
  );

  useEffect(() => {
    if (isTauri()) return;
    const id = window.setInterval(() => {
      setNowStr(new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }));
    }, 1_000);
    return () => window.clearInterval(id);
  }, []);

  // Browser-mode internet health state
  const [outages, setOutages] = useState<Outage[]>([]);

  // Browser-mode speed test state
  const [speedRunning, setSpeedRunning] = useState(false);
  const [speedResult, setSpeedResult] = useState<SpeedResult | null>(null);
  const [speedHistory, setSpeedHistory] = useState<SpeedResult[]>([]);

  useEffect(() => {
    if (!isTauri()) return;
    invoke<WifiInfoPayload>("get_wifi_info")
      .then(setWifiInfo)
      .catch(() => {});
  }, []);

  useEffect(() => {
    invoke<ApiNetwork[]>("get_networks")
      .then((data) => {
        if (Array.isArray(data) && data.length > 0) setApiNetwork(data[0]);
      })
      .catch(() => {});
  }, []);

  // Fetch outages and speed history in browser mode on mount
  useEffect(() => {
    if (isTauri()) return;
    invoke<Outage[]>("get_outages")
      .then((data) => { if (Array.isArray(data)) setOutages(data); })
      .catch(() => {});
    invoke<SpeedResult[]>("get_speed_test_history")
      .then((data) => { if (Array.isArray(data)) setSpeedHistory(data); })
      .catch(() => {});
  }, []);

  const handleRunSpeedTest = async () => {
    setSpeedRunning(true);
    try {
      const data = await invoke<SpeedResult>("run_speed_test");
      setSpeedResult(data);
      setSpeedHistory((prev) => [data, ...prev].slice(0, 30));
    } catch {}
    setSpeedRunning(false);
  };

  const ssidName = isTauri()
    ? (wifiInfo?.ssid ?? t("localNetwork"))
    : (apiNetwork?.ssid ?? t("unknown"));

  const wifiBadge = wifiInfo
    ? [wifiInfo.wifiStandard, wifiInfo.frequencyBand].filter(Boolean).join(" • ")
    : null;

  const localNetworkPill = useMemo(() => {
    const { isOnline, connectionType } = networkState;
    const offlineUi =
      !isOnline ||
      connectionType === "none" ||
      !lanScanAllowed;

    if (offlineUi) {
      return {
        label: (UI.offline as any)[lang] || UI.offline.en,
        className: "border-error/40 bg-error/10 text-error",
      };
    }
    if (connectionType === "wifi" || connectionType === "ethernet") {
      return {
        label: (UI.online as any)[lang] || UI.online.en,
        className: "border-online/30 bg-online/10 text-online",
      };
    }
    return {
      label: ((UI as any).unknown)[lang] || UI.unknown.en,
      className: "border-separator bg-surface-alt text-secondary",
    };
  }, [networkState, lanScanAllowed, lang]);

  const onlineDevices = useMemo(
    () => devices.filter((device) => {
      return device.lastSeen && (Date.now() - device.lastSeen < 5 * 60 * 1000);
    }),
    [devices],
  );
  const gatewayDevice = useMemo(() => {
    const valid = devices.filter(d => d.ip && d.ip.trim() !== "");
    return valid.find(d => d.ip === apiNetwork?.gateway) ||
           valid.find(d => d.likelyType === "Router / Gateway") ||
           valid.find(d => d.ip.endsWith(".1")) ||
           null;
  }, [devices, apiNetwork]);
  const recentOnlineDevices = useMemo(
    () => [...onlineDevices].slice(-5).reverse(),
    [onlineDevices],
  );
  const networkHealthLabel =
    !hasNetworkScoreData
      ? t("notMeasured")
      : networkScore >= 90
      ? t("excellent")
      : networkScore >= 70
        ? t("good")
        : t("needsAttention");

  // Internet health display values
  const currentOutage = outages.find((o) => o.ongoing) ?? null;
  const lastClosedOutage = outages.find((o) => !o.ongoing) ?? null;

  const internetStatusLine = currentOutage
    ? t("internetDown").replace("{time}", fmtTime(currentOutage.startedAt))
    : t("internetOnline");

  const lastOutageLine = lastClosedOutage
    ? t("lastOutage")
        .replace("{min}", String(fmtMins(lastClosedOutage.durationMs)))
        .replace("{days}", String(daysAgo(lastClosedOutage.startedAt)))
    : t("noOutages");

  // Derived values for browser stats row
  const internetOnline = currentOutage === null;
  const latestSpeedResult = speedResult ?? speedHistory[0] ?? null;
  const latestPingMs = latestSpeedResult?.pingMs ?? lastLatencyMs;

  // "Uptime" stat — count outages that started today
  const todayOutages = useMemo(() => {
    const midnight = new Date();
    midnight.setHours(0, 0, 0, 0);
    return outages.filter((o) => o.startedAt >= midnight.getTime() && !o.ongoing);
  }, [outages]);
  const todayOutageMinutes = todayOutages.reduce(
    (sum, o) => sum + fmtMins(o.durationMs),
    0,
  );
  const uptimeSubtext =
    todayOutages.length === 0
      ? "No outages today"
      : `${todayOutages.length} outage${todayOutages.length > 1 ? "s" : ""}, ${todayOutageMinutes} min total`;

  // "Last scan" ago string
  const lastScanAgo = useMemo(() => timeAgoStr(lastScanAt), [lastScanAt]);

  // Subnet derived from gateway
  const subnetCidr = gatewayToSubnet(apiNetwork?.gateway);

  return (
    <div className="min-h-screen bg-void">
      {/* ── NOC Status Bar (browser only) ── */}
      {!isTauri() && (
        <NocStatusBar
          nowStr={nowStr}
          internetOnline={internetOnline}
          onlineCount={onlineDevices.length}
          totalCount={devices.length}
          lastScanAgo={lastScanAgo}
        />
      )}

      <div className="space-y-6">
        {/* ── Page header ── */}
        <header className="hidden md:flex md:flex-col md:gap-2 lg:flex-row lg:items-end lg:justify-between">
          <div>
            {!isTauri() ? (
              <>
                <h2 className="text-3xl font-black tracking-tight text-primary uppercase">
                  Command Center
                </h2>
                <p className="mt-1 text-[11px] font-bold uppercase tracking-[0.3em] text-tertiary">
                  Node: WADDAN <span className="text-separator mx-2">|</span> Status: Real-time Monitoring
                  {subnetCidr ? ` · Net: ${subnetCidr}` : ""}
                </p>
              </>
            ) : (
              <>
                <h2 className="text-3xl font-semibold text-primary">
                  {t("networkOverview")}
                </h2>
                <p className="mt-1 text-sm text-secondary">
                  {t("networkOverviewDesc")}
                </p>
              </>
            )}
          </div>
        </header>

        {/* ── Top section: health shield + cards ── */}
        {!isTauri() ? (
          /* Browser mode: shield left + 4-column stats right */
          <section className="grid grid-cols-1 gap-6 lg:grid-cols-[280px_1fr] lg:items-stretch">
            {/* Health Shield */}
            <Card
              className={cn(
                "relative isolate overflow-hidden border-separator/40 bg-surface/20 ring-0",
                isScanning
                  ? "animate-health-shield-blue-scan"
                  : "animate-health-shield-glow",
              )}
            >
              <CardHeader className="relative z-10 pb-2 text-center">
                <CardDescription className="text-[10px] font-bold uppercase tracking-[0.25em] text-tertiary">
                  System Health
                </CardDescription>
                <CardTitle className="text-sm font-black uppercase tracking-widest text-primary">
                  Overall Posture
                </CardTitle>
              </CardHeader>
              <CardContent className="relative z-10 flex flex-col items-center gap-4 pb-8 pt-0">
                <Popover>
                  <PopoverTrigger className="mx-auto w-full max-w-sm rounded-2xl text-left outline-none transition hover:opacity-95 focus-visible:ring-2 focus-visible:ring-accent/60">
                    <NetworkHealthDisplay
                      score={networkScore}
                      hasNetworkScoreData={hasNetworkScoreData}
                    />
                  </PopoverTrigger>
                  <PopoverContent className="border-separator/40 bg-surface shadow-2xl">
                    <h3 className="text-[10px] font-black uppercase tracking-widest text-primary">
                      {t("scoreBreakdown")}
                    </h3>
                    <div className="mt-4 space-y-3 font-mono text-xs text-primary">
                      <div className="flex items-center justify-between">
                        <span className="text-tertiary uppercase tracking-tighter">{t("speed")}</span>
                        <span className="font-bold text-accent">
                          {hasNetworkScoreData ? `${networkScoreBreakdown.performance}/40` : "--/40"}
                        </span>
                      </div>
                      <div className="flex items-center justify-between">
                        <span className="text-tertiary uppercase tracking-tighter">{t("latency")}</span>
                        <span className="font-bold text-accent">
                          {hasNetworkScoreData ? `${networkScoreBreakdown.latency}/30` : "--/30"}
                        </span>
                      </div>
                      <div className="flex items-center justify-between border-b border-separator/30 pb-2">
                        <span className="text-tertiary uppercase tracking-tighter">{t("security")}</span>
                        <span className="font-bold text-accent">
                          {hasNetworkScoreData ? `${networkScoreBreakdown.security}/30` : "--/30"}
                        </span>
                      </div>
                      <p className="text-[9px] leading-relaxed text-tertiary italic">
                        {t("deductsNote")}
                      </p>
                    </div>
                  </PopoverContent>
                </Popover>
                <p className="text-xl font-black uppercase tracking-[0.2em] text-primary">
                  {networkHealthLabel}
                </p>
              </CardContent>
            </Card>

            {/* 4-column stats */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
              <BrowserStatCard
                label={dict.registeredNodes}
                value={devices.length}
                sub={`${onlineDevices.length} currently active`}
                subClass="text-online"
              />
              <div className="relative group">
                <BrowserStatCard
                  label="Internet Gateway"
                  value={
                    internetOnline
                      ? (gatewayDevice?.name || "LIVE")
                      : "DOWN"
                  }
                  sub={
                    internetOnline
                      ? (latestSpeedResult?.downloadMbps != null 
                          ? `↓${latestSpeedResult.downloadMbps.toFixed(0)} Mbps Effective` 
                          : (apiNetwork?.ssid || "Wan Uplink Active"))
                      : `Offline since ${currentOutage ? fmtTime(currentOutage.startedAt) : "—"}`
                  }
                  subClass={internetOnline ? "text-online" : "text-error"}
                />
                {internetOnline && apiNetwork?.gateway && (
                  <a
                    href={`http://${apiNetwork.gateway}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="absolute right-3 top-3 rounded border border-accent/30 bg-surface/90 px-2 py-1 text-[9px] font-bold uppercase tracking-widest text-accent opacity-0 transition-opacity hover:bg-accent hover:text-white group-hover:opacity-100"
                  >
                    {t("launchPortal")}
                  </a>
                )}
              </div>
              <BrowserStatCard
                label="Signal Latency"
                value={latestPingMs != null ? `${latestPingMs.toFixed(0)}ms` : "—"}
                sub="RTT to Edge Router"
                subClass={
                  latestPingMs == null
                    ? "text-tertiary"
                    : latestPingMs < 20
                      ? "text-online"
                      : latestPingMs < 80
                        ? "text-warning"
                        : "text-error"
                }
              />
              <BrowserStatCard
                label="Session Uptime"
                value={todayOutages.length === 0 ? "100%" : `${100 - todayOutages.length}%`}
                sub={uptimeSubtext}
                subClass={todayOutages.length === 0 ? "text-online" : "text-warning"}
              />
            </div>
          </section>
        ) : (
          /* Tauri mode: original xl:grid-cols-6 layout */
          <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-6 xl:items-stretch">
            <Card
              className={cn(
                "relative isolate overflow-hidden rounded-2xl ring-0 sm:col-span-2 xl:col-span-3",
                isScanning
                  ? "animate-health-shield-blue-scan"
                  : "animate-health-shield-glow",
              )}
            >
              <CardHeader className="relative z-10 text-center">
                <CardDescription className="text-xs uppercase tracking-wide text-secondary">
                  {t("healthShieldLabel")}
                </CardDescription>
                <CardTitle className="text-primary">
                  {t("healthShieldTitle")}
                </CardTitle>
              </CardHeader>
              <CardContent className="relative z-10 flex flex-col items-center gap-4 pb-8 pt-2">
                <Popover>
                  <PopoverTrigger className="mx-auto w-full max-w-sm rounded-2xl text-left outline-none ring-offset-2 ring-offset-void transition hover:opacity-95 focus-visible:ring-2 focus-visible:ring-accent/60">
                    <NetworkHealthDisplay
                      score={networkScore}
                      hasNetworkScoreData={hasNetworkScoreData}
                    />
                  </PopoverTrigger>
                  <PopoverContent>
                    <h3 className="text-sm font-semibold text-primary">
                      {t("scoreBreakdown")}
                    </h3>
                    <div className="mt-3 space-y-2 text-sm text-primary">
                      <p>
                        {t("speed")}{" "}
                        {hasNetworkScoreData
                          ? `${networkScoreBreakdown.performance}/40`
                          : "--/40"}
                      </p>
                      <p>
                        {t("latency")}{" "}
                        {hasNetworkScoreData
                          ? `${networkScoreBreakdown.latency}/30`
                          : "--/30"}
                        {lastLatencyMs !== null
                          ? ` (${Math.round(lastLatencyMs)}ms)`
                          : ` (${t("notMeasuredShort")})`}
                      </p>
                      <p>
                        {t("security")}{" "}
                        {hasNetworkScoreData
                          ? `${networkScoreBreakdown.security}/30`
                          : "--/30"}
                      </p>
                      <p className="pt-1 text-xs text-secondary">
                        {t("deductsNote")}
                      </p>
                    </div>
                  </PopoverContent>
                </Popover>
                <p className="mt-6 text-lg font-semibold text-primary">
                  {networkHealthLabel}
                </p>
                <p className="text-center text-sm text-secondary">
                  {t("shieldHint")}
                </p>
              </CardContent>
            </Card>

            {/* Network Status: Tauri only */}
            <Card className="xl:col-span-1">
              <CardHeader>
                <CardDescription className="text-xs uppercase tracking-wide text-secondary">
                  {t("networkStatus")}
                </CardDescription>
                <CardTitle className="flex items-center gap-2 text-2xl text-primary">
                  <Wifi className="size-5 shrink-0 text-secondary" aria-hidden />
                  <span className="truncate">{ssidName}</span>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-2">
                <div
                  className={cn(
                    "inline-flex items-center gap-2 rounded-full border px-3 py-1 text-sm font-medium",
                    localNetworkPill.className,
                  )}
                >
                  <Signal className="size-4" aria-hidden />
                  {localNetworkPill.label}
                </div>
                {wifiBadge && (
                  <p className="text-xs text-secondary">{wifiBadge}</p>
                )}
              </CardContent>
            </Card>

            <Card className="xl:col-span-1">
              <CardHeader>
                <CardDescription className="text-xs uppercase tracking-wide text-secondary">
                  {t("devicesLabel")}
                </CardDescription>
                <CardTitle className="text-4xl text-accent">
                  {devices.length}
                </CardTitle>
              </CardHeader>
              <CardContent className="text-sm text-secondary">
                {t("onlineCount").replace("{n}", String(onlineDevices.length))}
              </CardContent>
            </Card>

            {/* Speed Test card — Tauri */}
            <Card className="xl:col-span-1">
              <CardHeader>
                <CardDescription className="text-xs uppercase tracking-wide text-secondary">
                  {t("lastSpeedTest")}
                </CardDescription>
                <CardTitle className="text-4xl text-accent">
                  {lastSpeedMbps !== null ? `${lastSpeedMbps.toFixed(1)} Mbps` : "--"}
                </CardTitle>
              </CardHeader>
              <CardContent className="text-sm text-secondary">
                {lastSpeedTestAt
                  ? t("lastTestedAt").replace("{time}", lastSpeedTestAt.toLocaleTimeString())
                  : t("noSpeedTestYet")}
              </CardContent>
            </Card>
          </section>
        )}

        {/* ── Topology Map ── */}
        <section className="h-[500px] min-h-[400px] w-full overflow-hidden rounded-2xl border border-separator/40 bg-void relative shadow-2xl">
          <TopologyMap />
        </section>

        {/* ── Bottom section ── */}
        <section className="grid gap-4 lg:grid-cols-3">
          {/* Left column: speed test / performance */}
          <div className="space-y-3 lg:col-span-2">
            {!isTauri() ? (
              <BrowserSpeedSection
                speedRunning={speedRunning}
                speedResult={speedResult}
                speedHistory={speedHistory}
                lastLatencyMs={lastLatencyMs}
                onRunSpeedTest={handleRunSpeedTest}
              />
            ) : (
              <NetworkPerformanceSection />
            )}
          </div>

          {/* Right column: internet health + outage history + recent activity */}
          <div className="space-y-4">
            {/* Internet Health — browser mode only */}
            {!isTauri() && (
              <Card>
                <CardHeader>
                  <CardDescription className="text-xs uppercase tracking-wide text-secondary">
                    {t("internetHealth")}
                  </CardDescription>
                  <CardTitle
                    className={cn(
                      "text-base font-semibold",
                      currentOutage ? "text-error" : "text-online",
                    )}
                  >
                    {internetStatusLine}
                  </CardTitle>
                </CardHeader>
                <CardContent className="text-sm text-secondary">
                  {lastOutageLine}
                </CardContent>
              </Card>
            )}

            {/* Outage history — browser mode only */}
            {!isTauri() && <OutageHistoryCard outages={outages} />}

            {/* Recent activity — always shown */}
            <Card className="bg-surface/30 border-separator/40">
              <CardHeader className="pb-4">
                <CardTitle className="text-[10px] font-black uppercase tracking-[0.2em] text-primary">{t("recentActivity")}</CardTitle>
                <CardDescription className="text-[9px] font-bold uppercase tracking-widest text-tertiary">
                  {t("recentActivityDesc")}
                </CardDescription>
              </CardHeader>
              <CardContent>
                <div className="max-h-[400px] space-y-2 overflow-y-auto pr-1">
                  {recentOnlineDevices.length > 0 ? (
                    recentOnlineDevices.map((device) => (
                      <div
                        key={`${device.mac}-${device.ip}`}
                        className="rounded-lg border border-separator/20 bg-void/40 p-3 transition-colors hover:bg-surface-alt/20"
                      >
                        <div className="flex items-center justify-between">
                          <p className="font-bold text-[13px] text-primary truncate">{device.name}</p>
                          <span className="h-1.5 w-1.5 rounded-full bg-online shadow-[0_0_6px_rgba(48,209,88,0.4)]" />
                        </div>
                        <p className="font-mono text-[10px] text-secondary mt-1">
                          {device.ip} <span className="text-tertiary mx-1">·</span> {device.mac}
                        </p>
                        <p className="mt-2 text-[9px] font-black uppercase tracking-widest text-online">
                          {device.vendorName}
                        </p>
                      </div>
                    ))
                  ) : (
                    <div className="rounded-md border border-dashed border-separator/40 bg-void/20 p-6 text-center">
                      <p className="text-[10px] font-bold uppercase tracking-widest text-tertiary">
                        {t("noRecentDevices")}
                      </p>
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>
          </div>
        </section>
      </div>
    </div>
  );
}

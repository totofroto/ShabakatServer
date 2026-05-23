import {
  ChevronRight,
  CircleCheck,
  Loader2,
  Network,
  Radar,
  ScanSearch,
  Shield,
  ShieldCheck,
  Zap,
} from "lucide-react";
import { invoke, isTauri } from "@/lib/transport";
import { listen } from "@/lib/transport";
import { vibrate } from "@tauri-apps/plugin-haptics";
import {
  Fragment,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useLanguage, type AppLang } from "@/context/LanguageContext";
import { useNetworkConnectivity } from "@/context/NetworkConnectivityContext";
import { useScanContext } from "@/context/ScanContext";
import {
  addLatencySample,
  getLatencySamples,
  isValidMacForWakeOnLan,
  type DeviceRow,
  type ScanMode,
} from "@/hooks/useNetworkScan";
import arStrings from "@/i18n/ar.json";
import { cn } from "@/lib/utils";

const UI: Record<string, { en: string; ar: string; de?: string }> = {
  pageTitle:      { en: "Devices",                           ar: "الأجهزة" },
  pageDesc:       { en: "Discover hosts on your local network via the Shabakat scanner.", ar: "اكتشف الأجهزة على شبكتك المحلية عبر ماسح Shabakat." },  scanNetwork:    { en: "Scan Network",                      ar: "مسح الشبكة" },
  scanning:       { en: "Scanning",                          ar: "جارٍ المسح" },
  listView:       { en: "List View",                         ar: "قائمة" },
  starMapView:    { en: "Star-Map View",                     ar: "خريطة النجوم" },
  networkDevices: { en: "Network Devices",                   ar: "أجهزة الشبكة" },
  discovered:     { en: "Discovered",                        ar: "تم اكتشاف" },
  liveHosts:      { en: "live hosts on your network.",       ar: "جهازاً نشطاً على شبكتك." },
  tapRadar:       { en: "Tap the Radar to begin discovery.", ar: "اضغط على الرادار لبدء الاكتشاف." },
  detailsArrow:   { en: "Details →",                        ar: "← التفاصيل" },
  hostname:       { en: "Hostname",                          ar: "اسم المضيف" },
  ipAddress:      { en: "IP Address",                        ar: "عنوان IP" },
  macAddress:     { en: "MAC Address",                       ar: "عنوان MAC" },
  vendor:         { en: "Vendor",                            ar: "الشركة المصنعة" },
  deviceType:     { en: "Device Type",                       ar: "نوع الجهاز" },
  likelyType:     { en: "Likely Type",                       ar: "النوع المرجح" },
  activeId:       { en: "Active ID",                         ar: "المعرف النشط" },
  privacy:        { en: "Privacy",                           ar: "الخصوصية" },
  privateWifi:    { en: "Private Wi-Fi Address",             ar: "عنوان Wi-Fi خاص" },
  standardMac:    { en: "Standard MAC",                      ar: "MAC قياسي" },
  identityVault:  { en: "Identity Vault",                    ar: "قُبة الهوية" },
  setCustomName:  { en: "Set Custom Name",                   ar: "تعيين اسم مخصص" },
  editName:       { en: "Edit Name",                         ar: "تعديل الاسم" },
  save:           { en: "Save",                              ar: "حفظ" },
  cancel:         { en: "Cancel",                            ar: "إلغاء" },
  clear:          { en: "Clear",                             ar: "مسح" },
  noCustomName:   { en: "No custom name set.",               ar: "لم يُعيَّن اسم مخصص." },
  rawLogs:        { en: "Raw Interrogation Log",             ar: "سجل الاستجواب الخام" },
  portGuardian:   { en: "Port Guardian",                     ar: "حارس المنافذ" },
  rerunPortScan:  { en: "Re-run Port Guardian",              ar: "إعادة تشغيل حارس المنافذ" },
  deepScanAll:    { en: "Deep Scan All",                      ar: "فحص عميق للجميع" },
  deepScanningAll:{ en: "Deep scanning all...",               ar: "جارٍ الفحص العميق للجميع..." },
  openPorts:      { en: "Open Ports",                        ar: "المنافذ المفتوحة" },
  custom:         { en: "Custom",                            ar: "مخصص" },
  noDevicesFound: { en: "No devices to deep scan yet.",      ar: "لا توجد أجهزة للفحص العميق بعد." },
  currentNetworkKicker: { en: "Current Network",          ar: "الشبكة الحالية" },
  readyToScan: { en: "Ready to scan",                        ar: "جاهز للفحص" },
  readyToScanHint: { en: "Tap the radar to discover devices on this LAN.", ar: "اضغط على الرادار لاكتشاف الأجهزة على هذه الشبكة." },
  unknownRouter: { en: "Unknown Router",                    ar: "موجّه غير معروف" },
  gatewayWithIp: { en: "Gateway: {ip}",                    ar: "البوابة: {ip}" },
  noGatewayLine: { en: "Router not identified on this pass.", ar: "لم يُعرَف الموجّه في هذه الجولة." },
  onlineVsTotalPill: { en: "{online} online / {total} known", ar: "{online} متصل / {total} معروف" },
  thisNetworkTitle: { en: "This network",                 ar: "هذه الشبكة" },
  deepScanDone:   { en: "Deep scan completed.",              ar: "اكتمل الفحص العميق." },
  deepScanFailed: { en: "Deep scan all failed.",             ar: "فشل الفحص العميق للجميع." },
  deviceInspectorDesc: { en: "Port Guardian and live latency for this host.", ar: "حارس المنافذ وزمن الاستجابة المباشر لهذا الجهاز." },
  forensicInProgress: { en: "Forensic Scan in Progress…", ar: "جارٍ الفحص الجنائي..." },
  scanningPorts: { en: "Scanning Ports…", ar: "جارٍ فحص المنافذ..." },
  criticalRisk: { en: "Critical risk:", ar: "خطر حرج:" },
  warning: { en: "Warning:", ar: "تحذير:" },
  secure: { en: "Secure:", ar: "آمن:" },
  criticalRiskBody: { en: "SSH (22) or RDP (3389) is reachable from the LAN. Trust score reduced by 30.", ar: "يمكن الوصول إلى SSH ‏(22) أو RDP ‏(3389) من الشبكة المحلية. تم خفض درجة الثقة بمقدار 30." },
  httpWarningBody: { en: "HTTP (80) is open without HTTPS (443) — cleartext web exposure.", ar: "منفذ HTTP ‏(80) مفتوح بدون HTTPS ‏(443) — تعرض ويب بنص واضح." },
  secureBody: { en: "HTTPS (443) present without cleartext HTTP on this audit.", ar: "تم العثور على HTTPS ‏(443) بدون HTTP بنص واضح في هذا التدقيق." },
  automatedSweepFlagged: { en: "Automated sweep flagged SSH (22), RDP (3389), or HTTP (80). Review exposure on this host.", ar: "الفحص الآلي رصد SSH ‏(22) أو RDP ‏(3389) أو HTTP ‏(80). راجع مستوى التعرض لهذا الجهاز." },
  runForensicSweep: { en: "Run a forensic port sweep from this panel to audit common services.", ar: "شغّل فحص منافذ جنائي من هذه اللوحة لتدقيق الخدمات الشائعة." },
  customNamePlaceholder: { en: "e.g. \"Tareg's PC\"", ar: "مثال: \"حاسوب طارق\"" },
  livePingUnavailable: { en: "Live ping is unavailable while this device is offline.", ar: "القياس المباشر غير متاح أثناء عدم اتصال هذا الجهاز." },
  checkingAuditPorts: { en: "Checking curated audit ports…", ar: "جارٍ فحص منافذ التدقيق المحددة..." },
  noOpenPortsPass: { en: "No open ports from the Port Guardian list on this pass.", ar: "لا توجد منافذ مفتوحة من قائمة حارس المنافذ في هذه الجولة." },
  unknownService: { en: "Unknown Service", ar: "خدمة غير معروفة" },
  exposureReview: { en: "Exposure: {count} open port{plural} — review critical services.", ar: "تعرض: {count} منفذ مفتوح{plural} — راجع الخدمات الحرجة." },
  foundOpenPorts: { en: "Found {count} open audit port{plural}.", ar: "تم العثور على {count} منفذ تدقيق مفتوح{plural}." },
  copyIpAddress: { en: "IP Address Copied", ar: "تم نسخ عنوان IP" },
  copyFailed: { en: "Copy failed", ar: "فشل النسخ" },
  dismiss: { en: "Dismiss", ar: "إغلاق" },
  scanningNetworkProgress: { en: "Scanning your network... {pct}%", ar: "جارٍ فحص الشبكة... {pct}%" },
  scanningHostProgress: { en: "Scanning {done}/{total}...", ar: "جارٍ الفحص {done}/{total}..." },
  finalizing: { en: "Finalizing...", ar: "جارٍ الإنهاء..." },
  scanningSimple: { en: "Scanning...", ar: "جارٍ الفحص..." },
  scanningTimeRemaining: { en: "Scanning... ~{sec}s left", ar: "جارٍ الفحص... متبقٍ ~{sec} ثانية" },
  scanModeLabel: { en: "Scan Mode", ar: "وضع الفحص" },
  silentMode: { en: "Silent Detective", ar: "حارس هادئ" },
  silentModeDesc: { en: "Bypasses firewalls on public networks.", ar: "يتجاوز الجدران النارية في الشبكات العامة." },
  aggressiveMode: { en: "Aggressive", ar: "فحص هجومي" },
  aggressiveModeDesc: { en: "High-speed discovery for home networks.", ar: "اكتشاف سريع للشبكات المنزلية." },
  scanSilent: { en: "Silent Scan", ar: "فحص هادئ" },
  scanAggressive: { en: "Aggressive Scan", ar: "فحص هجومي" },
  silentModeBadge: { en: "Silent Mode", ar: "وضع الصامت" },
  aggressiveModeBadge: { en: "Aggressive Mode", ar: "وضع الهجوم" },
  largeNetworkHint: {
    en: "Scanning large network... this may take 60 seconds.",
    ar: "جارٍ فحص شبكة كبيرة... قد يستغرق هذا 60 ثانية.",
  },
  cancelScan: { en: "Cancel Scan", ar: "إلغاء الفحص" },
  unknownDevice: { en: "Unknown Device", ar: "جهاز غير معروف" },
  likely: { en: "Likely", ar: "مرجح" },
  deviceDetailsDashboard: { en: "Device details", ar: "تفاصيل الجهاز" },
  wakeOnLan: { en: "Wake on LAN", ar: "إيقاظ عبر LAN" },
  wakeDeviceList: { en: "Wake device", ar: "إيقاظ الجهاز" },
  wakeDeviceSending: { en: "Sending…", ar: "جارٍ الإرسال…" },
  runDeepScanAction: { en: "Run Deep Scan", ar: "فحص عميق" },
  fingerprintNotAvailable: { en: "Not yet fingerprinted", ar: "لم يُستخرج البصمة بعد" },
  wakeOnLanNeedMac: {
    en: "Wake on LAN needs a valid hardware MAC (not Unknown or all-zero).",
    ar: "إيقاظ LAN يتطلب عنوان MAC صالحاً (ليس غير معروف أو كله أصفار).",
  },
  deepScanNeedIp: {
    en: "Deep scan requires a valid IPv4 address.",
    ar: "الفحص العميق يتطلب عنوان IPv4 صالحاً.",
  },
  deepScanPortsProgress: {
    en: "{checked} / {total} ports checked",
    ar: "تم فحص {checked} / {total} منفذ",
  },
  deepScanOpenPortsTitle: {
    en: "Open ports (live)",
    ar: "منافذ مفتوحة (مباشر)",
  },
  macHiddenByOs: { en: "MAC Hidden by OS", ar: "MAC مخفي من نظام التشغيل" },
  resolvingMac:  { en: "Resolving MAC…",   ar: "جارٍ حل MAC..." },
  filterAll:     { en: "All",              ar: "الكل" },
  filterOnline:  { en: "Online",           ar: "متصل" },
  filterOffline: { en: "Offline",          ar: "غير متصل" },
  filterNew:     { en: "New",              ar: "جديد" },
  hideStaleLabel: { en: "Hide stale (>7d)", ar: "إخفاء القديمة (>7أيام)" },
  showStaleLabel: { en: "Show stale ({n})", ar: "إظهار القديمة ({n})" },
  launchPortal: { en: "Launch Portal", ar: "فتح البوابة", de: "Portal öffnen" },
};
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { DeviceDetailsPanel } from "@/components/device-details-panel";
import {
  DeviceLastSeenLabel,
  DeviceLaunchPortalLink,
  DeviceListEditButton,
  DeviceListNameHeading,
  DeviceListWakeButton,
  DeviceOfflineBadge,
  DeviceRowShell,
  DeviceStatusDot,
  DeviceTypeIcon,
  getDeviceListMetaLine,
  getDeviceListPrimaryLine,
} from "@/components/device-row";
import { NetworkStarMap } from "@/components/NetworkStarMap";



function deviceMacResolved(d: Pick<DeviceRow, "mac">): boolean {
  const m = d.mac?.trim() ?? "";
  return Boolean(m && m !== "Unknown" && m !== "MAC Restricted");
}

/** True when we have a real hostname (Port Guardian, mDNS, reverse-DNS, or `name`). */
function deviceHostnameIdentified(d: DeviceRow): boolean {
  if (d.interrogationName?.trim()) {
    return true;
  }
  if (d.hostname?.trim()) {
    return true;
  }
  if (d.mdnsHostname?.trim()) {
    return true;
  }
  const n = (d.name ?? "").trim();
  if (!n || n.toLowerCase() === "unknown") {
    return false;
  }
  const low = n.toLowerCase();
  if (low.startsWith("host ")) {
    return false;
  }
  if (n === d.ip) {
    return false;
  }
  return true;
}

function deviceOuiManufacturerKnown(d: DeviceRow): boolean {
  const v = d.vendorName?.trim() ?? "";
  return Boolean(v && v !== "Unknown");
}

function deviceIdentityScore(d: DeviceRow): {
  score: number;
  kind: "trust" | "identified" | "warning";
} {
  const mac = deviceMacResolved(d);
  const host = deviceHostnameIdentified(d);
  if (mac && host) {
    return { score: 100, kind: "trust" };
  }
  if (mac && deviceOuiManufacturerKnown(d)) {
    return { score: 75, kind: "identified" };
  }
  return { score: 40, kind: "warning" };
}

type DeviceForensicRecord = {
  scanned: boolean;
  openPorts: number[];
};

function forensicCriticalPorts(openPorts: number[]): boolean {
  const s = new Set(openPorts);
  return s.has(22) || s.has(3389);
}

function displayTrustAfterForensic(
  device: DeviceRow,
  forensic?: DeviceForensicRecord,
): number {
  const base = deviceIdentityScore(device).score;
  if (!forensic?.scanned) {
    return base;
  }
  if (forensicCriticalPorts(forensic.openPorts)) {
    return Math.max(0, base - 30);
  }
  return base;
}


const PORT_LABELS: Record<number, string> = {
  21: "FTP",
  22: "SSH",
  23: "Telnet",
  25: "SMTP",
  53: "DNS",
  80: "HTTP",
  110: "POP3",
  135: "RPC",
  139: "NetBIOS",
  443: "HTTPS",
  445: "SMB",
  3306: "MySQL",
  3389: "RDP",
  5000: "UPnP / alt-HTTP",
  5432: "PostgreSQL",
  8080: "HTTP Alt",
  8443: "HTTPS Alt",
};

// ── Identity Vault ────────────────────────────────────────────────────────────

const CUSTOM_NAMES_KEY = "shabakat_custom_names";

function readCustomNames(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(CUSTOM_NAMES_KEY) ?? "{}") as Record<string, string>;
  } catch {
    return {};
  }
}

function useCustomNames() {
  const [names, setNames] = useState<Record<string, string>>(readCustomNames);

  const saveName = useCallback((ip: string, name: string) => {
    setNames((prev) => {
      const next = { ...prev, [ip]: name.trim() };
      if (!name.trim()) {
        delete next[ip];
      }
      localStorage.setItem(CUSTOM_NAMES_KEY, JSON.stringify(next));
      return next;
    });
  }, []);

  return { names, saveName };
}

// ── Toast ─────────────────────────────────────────────────────────────────────

function useToast() {
  const [message, setMessage] = useState<string | null>(null);
  const timerRef = useRef<number | null>(null);

  const showToast = useCallback((msg: string) => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
    }
    setMessage(msg);
    timerRef.current = window.setTimeout(() => {
      setMessage(null);
      timerRef.current = null;
    }, 2400);
  }, []);

  useEffect(() => () => {
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
  }, []);

  return { message, showToast };
}

function ToastNotification({ message }: { message: string | null }) {
  return (
    <div
      aria-live="polite"
      aria-atomic="true"
      className={cn(
        "pointer-events-none fixed bottom-[calc(7rem+env(safe-area-inset-bottom))] left-1/2 z-[60] -translate-x-1/2 rounded-full border border-separator bg-surface px-5 py-2.5 text-sm font-semibold text-primary transition-all duration-300",
        message ? "translate-y-0 opacity-100" : "translate-y-3 opacity-0",
      )}
    >
      {message ?? ""}
    </div>
  );
}

// ── Network HUD ───────────────────────────────────────────────────────────────

type NetworkInfo = {
  localIp: string | null;
  subnetCidr: string | null;
  gatewayIp: string | null;
};

function NetworkHUD({ info }: { info: NetworkInfo | null }) {
  if (!info?.subnetCidr) {
    return null;
  }
  return (
    <div className="mt-4 flex flex-wrap items-center gap-x-5 gap-y-1.5 rounded-xl border border-separator bg-surface px-4 py-2.5 text-xs font-mono">
      <span className="text-secondary uppercase tracking-widest text-[10px] font-bold">Subnet</span>
      <span className="text-accent font-semibold">{info.subnetCidr}</span>
      {info.gatewayIp ? (
        <>
          <span className="text-tertiary">·</span>
          <span className="text-secondary uppercase tracking-widest text-[10px] font-bold">Gateway</span>
          <span className="text-accent font-semibold">{info.gatewayIp}</span>
        </>
      ) : null}
      {info.localIp ? (
        <>
          <span className="text-tertiary">·</span>
          <span className="text-secondary uppercase tracking-widest text-[10px] font-bold">This device</span>
          <span className="text-accent font-semibold">{info.localIp}</span>
        </>
      ) : null}
    </div>
  );
}

type PortScanPayload = {
  openPorts: number[];
  likelyType?: string | null;
  interrogationName?: string | null;
};

type DeepScanProgressPayload = {
  ip: string;
  openPorts: number[];
  likelyType: string;
};

function rowKey(d: { ip: string; mac: string }): string {
  const m = d.mac?.trim();
  if (m && m !== "Unknown") {
    return m;
  }
  return d.ip;
}

function CommandCenterHealthShield({
  score,
  hasData,
  scanActive,
}: {
  score: number;
  hasData: boolean;
  scanActive: boolean;
}) {
  const display =
    hasData && Number.isFinite(score)
      ? Math.min(Math.max(Math.round(score), 0), 100)
      : "--";

  return (
    <div
      className={cn(
        "relative isolate mb-4 shrink-0 overflow-hidden rounded-2xl border px-5 py-3 transition-[border-color] duration-300",
        scanActive
          ? "animate-health-shield-blue-scan border-accent/50 bg-surface"
          : "border-accent/20 bg-surface",
      )}
      aria-live="polite"
    >
      <p className="relative z-10 text-center text-[10px] font-bold uppercase tracking-[0.28em] text-accent">
        Health Shield
      </p>
      <p className="relative z-10 mt-1 text-center text-4xl font-black tabular-nums tracking-tight text-primary">
        {display}
      </p>
      <p className="relative z-10 mt-0.5 text-center text-[10px] uppercase tracking-wider text-secondary">
        Posture score
      </p>
    </div>
  );
}

/** Z Fold inner / tablet split — `lg` breakpoint (1024px). */
function useFoldableWideLayout(): boolean {
  const [wide, setWide] = useState(() =>
    typeof window !== "undefined" ? window.innerWidth >= 1024 : false,
  );

  useEffect(() => {
    const update = () => setWide(window.innerWidth >= 1024);
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

  return wide;
}

function PingLatencyChart({
  samples,
  animateLive = false,
}: {
  samples: number[];
  animateLive?: boolean;
}) {
  const W = 280;
  const H = 56;
  const PAD = { top: 4, right: 4, bottom: 4, left: 4 };
  const CW = W - PAD.left - PAD.right;
  const CH = H - PAD.top - PAD.bottom;

  if (samples.length === 0) {
    return (
      <div className="bg-surface rounded-xl p-4">
        <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider mb-3">
          Live Ping
        </p>
        <div className="flex h-14 items-center justify-center">
          <span className="text-[13px] text-tertiary">Gathering samples…</span>
        </div>
      </div>
    );
  }

  const max = Math.max(...samples, 10);
  const min = Math.min(...samples);
  const span = Math.max(max - min, 5);
  const latest = samples[samples.length - 1];
  const latencyClass =
    latest < 20 ? "text-online" : latest < 80 ? "text-warning" : "text-error";

  const toX = (i: number) =>
    PAD.left + (samples.length <= 1 ? CW / 2 : (i / (samples.length - 1)) * CW);
  const toY = (v: number) =>
    PAD.top + CH - ((v - min) / span) * CH;

  const linePts = samples.map((v, i) => `${toX(i).toFixed(2)},${toY(v).toFixed(2)}`);
  const linePath = `M ${linePts.join(" L ")}`;
  const bottomY = (PAD.top + CH).toFixed(2);
  const fillPath = `M ${toX(0).toFixed(2)},${bottomY} L ${linePts.join(" L ")} L ${toX(samples.length - 1).toFixed(2)},${bottomY} Z`;

  const gridYs = [0.25, 0.5, 0.75].map((pct) => PAD.top + CH * (1 - pct));

  return (
    <div className={cn("bg-surface rounded-xl p-4", animateLive && "pulse-live")}>
      <div className="flex items-center justify-between mb-3">
        <span className="text-[13px] font-semibold text-secondary uppercase tracking-wider">
          Live Ping
        </span>
        <span className={cn("text-[15px] font-medium font-[tabular-nums]", latencyClass)}>
          {latest.toFixed(1)} ms
        </span>
      </div>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="w-full"
        preserveAspectRatio="none"
        aria-hidden
      >
        {gridYs.map((y, i) => (
          <line
            key={i}
            x1={PAD.left}
            y1={y.toFixed(2)}
            x2={W - PAD.right}
            y2={y.toFixed(2)}
            stroke="var(--chart-grid)"
            strokeWidth="1"
          />
        ))}
        <path d={fillPath} fill="var(--chart-fill)" />
        <path
          d={linePath}
          stroke="var(--chart-line)"
          strokeWidth="2"
          fill="none"
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
      <div className="flex justify-between mt-1">
        <span className="text-[11px] text-secondary font-[tabular-nums]">
          {min.toFixed(0)} ms
        </span>
        <span className="text-[11px] text-secondary font-[tabular-nums]">
          {max.toFixed(0)} ms
        </span>
      </div>
    </div>
  );
}


type DeviceInspectorProps = {
  device: DeviceRow;
  pingSamples: number[];
  isPortScanRunning: boolean;
  forensicAutoScanning: boolean;
  portScanProgress: number;
  openPorts: number[];
  portScanError: string | null;
  likelyType: string | null;
  interrogationName: string | null;
  onRunPortScan: () => void | Promise<void>;
  showSheetChrome?: boolean;
  customName: string | null;
  onSaveCustomName: (name: string) => void;
  onCopyIp: (ip: string) => void;
  lang: AppLang;
  };


function DeviceInspector({
  device,
  pingSamples,
  isPortScanRunning,
  forensicAutoScanning,
  portScanProgress,
  openPorts,
  portScanError,
  likelyType: _likelyType,
  interrogationName: _interrogationName,
  onRunPortScan,
  showSheetChrome = false,
  customName: _customName,
  onSaveCustomName: _onSaveCustomName,
  onCopyIp: _onCopyIp,
  lang,
}: DeviceInspectorProps) {
  const t = (key: keyof typeof UI): string => (UI[key] as any)[lang] || UI[key].en;
  const forensicBusy = forensicAutoScanning || isPortScanRunning;
  const criticalOpen = openPorts.includes(22) || openPorts.includes(3389);
  const httpCleartextWarn = openPorts.includes(80) && !openPorts.includes(443);
  const httpsOnlySecure =
    openPorts.includes(443) && !openPorts.includes(80) && !criticalOpen;

  const postureTitle = forensicBusy
    ? t("portGuardian")
    : criticalOpen
      ? t("criticalRisk").replace(":", "")
      : httpCleartextWarn
        ? t("warning").replace(":", "")
        : httpsOnlySecure
          ? t("secure").replace(":", "")
          : t("portGuardian");

  const postureBody = forensicBusy
    ? t("forensicInProgress")
    : criticalOpen
      ? t("criticalRiskBody")
      : httpCleartextWarn
        ? t("httpWarningBody")
        : httpsOnlySecure
          ? t("secureBody")
          : device.shieldHighlight
            ? t("automatedSweepFlagged")
            : t("runForensicSweep");

  return (
    <div>
      {/* Accessible sheet title — visually hidden, required by shadcn Sheet */}
      {showSheetChrome && (
        <SheetHeader className="sr-only">
          <SheetTitle>{device.name || device.ip}</SheetTitle>
          <SheetDescription>{t("deviceInspectorDesc")}</SheetDescription>
        </SheetHeader>
      )}

      {/* ── PORT GUARDIAN ── */}
      <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
        {t("portGuardian")}
      </p>
      <div className="bg-surface rounded-xl overflow-hidden mx-4">
        {/* Security posture row — Section 5.10 style */}
        <div className="flex items-start justify-between px-4 py-3">
          <div className="flex-1 min-w-0 pr-3">
            <p className="text-[15px] font-semibold text-primary">{postureTitle}</p>
            <p className="text-[13px] text-secondary mt-0.5 leading-snug">{postureBody}</p>
          </div>
          {forensicBusy ? (
            <Loader2 className="w-5 h-5 text-secondary animate-spin shrink-0 mt-0.5" aria-hidden />
          ) : criticalOpen ? (
            <Shield className="w-5 h-5 text-error shrink-0 mt-0.5" aria-hidden />
          ) : httpCleartextWarn ? (
            <Shield className="w-5 h-5 text-warning shrink-0 mt-0.5" aria-hidden />
          ) : httpsOnlySecure ? (
            <ShieldCheck className="w-5 h-5 text-online shrink-0 mt-0.5" aria-hidden />
          ) : (
            <Shield className="w-5 h-5 text-tertiary shrink-0 mt-0.5" aria-hidden />
          )}
        </div>
        {/* Scan progress bar */}
        {forensicBusy && (
          <>
            <div className="h-px bg-separator ml-4" />
            <div className="px-4 py-3">
              <div className="h-1 w-full overflow-hidden rounded-full bg-surface-alt">
                <div
                  className="h-full rounded-full bg-accent transition-[width] duration-150 ease-out"
                  style={{ width: `${portScanProgress}%` }}
                />
              </div>
            </div>
          </>
        )}
        {portScanError && (
          <>
            <div className="h-px bg-separator ml-4" />
            <div className="px-4 py-3">
              <p className="text-[13px] text-error">{portScanError}</p>
            </div>
          </>
        )}
      </div>

      {/* Re-run button */}
      <div className="px-4 mt-2.5">
        <button
          type="button"
          onClick={onRunPortScan}
          disabled={forensicBusy}
          className={cn(
            "w-full py-3.5 rounded-xl bg-surface-alt text-primary text-[16px] font-semibold text-center transition-colors",
            "hover:bg-surface-hover disabled:opacity-50 disabled:cursor-not-allowed",
          )}
        >
          {forensicBusy ? (
            <span className="inline-flex items-center justify-center gap-2">
              <Loader2 className="w-4 h-4 animate-spin" aria-hidden />
              {forensicAutoScanning ? t("forensicInProgress") : t("scanningPorts")}
            </span>
          ) : (
            t("rerunPortScan")
          )}
        </button>
      </div>

      {/* ── OPEN PORTS ── */}
      {openPorts.length > 0 && !forensicBusy && (
        <>
          <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
            {t("openPorts")}
          </p>
          <div className="bg-surface rounded-xl overflow-hidden mx-4">
            {openPorts.map((port, i) => {
              const isCritical = port === 22 || port === 3389;
              const isWarn = port === 80 && !openPorts.includes(443);
              const isSecure =
                port === 443 &&
                !openPorts.includes(80) &&
                !openPorts.includes(22) &&
                !openPorts.includes(3389);
              return (
                <div key={port}>
                  {i > 0 && <div className="h-px bg-separator ml-4" />}
                  <div className="flex items-start justify-between px-4 py-3">
                    <div>
                      <p className="text-[15px] font-semibold text-primary font-mono">{port}</p>
                      <p className="text-[13px] text-secondary mt-0.5">
                        {PORT_LABELS[port] ?? t("unknownService")}
                      </p>
                    </div>
                    {isCritical ? (
                      <Shield className="w-5 h-5 text-error shrink-0 mt-0.5" aria-hidden />
                    ) : isWarn ? (
                      <Shield className="w-5 h-5 text-warning shrink-0 mt-0.5" aria-hidden />
                    ) : isSecure ? (
                      <ShieldCheck className="w-5 h-5 text-online shrink-0 mt-0.5" aria-hidden />
                    ) : (
                      <CircleCheck className="w-5 h-5 text-tertiary shrink-0 mt-0.5" aria-hidden />
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </>
      )}
      {openPorts.length === 0 && !forensicBusy && (
        <>
          <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
            {t("openPorts")}
          </p>
          <div className="bg-surface rounded-xl overflow-hidden mx-4">
            <div className="px-4 py-3">
              <span className="text-[15px] text-secondary">{t("noOpenPortsPass")}</span>
            </div>
          </div>
        </>
      )}

      {/* ── LATENCY ── */}
      <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
        Latency
      </p>
      <div className="mx-4">
        {device.status === "Online" ? (
          <PingLatencyChart samples={pingSamples} animateLive />
        ) : (
          <div className="bg-surface rounded-xl overflow-hidden">
            <div className="px-4 py-3">
              <span className="text-[15px] text-secondary">{t("livePingUnavailable")}</span>
            </div>
          </div>
        )}
      </div>

      <div className="h-6" />
    </div>
  );
}

type DevicesViewMode = "list" | "starmap";

export function DevicesPage() {
  const {
    devices,
    isScanning,
    isLoading,
    progressPct,
    scannedHosts,
    totalHosts,
    averageLatencyMs,
    triggerScan,
    cancelScan,
    patchDevice,
    scanPermissionError,
    clearScanPermissionError,
    scanRuntimeError,
    clearScanRuntimeError,
    networkScore,
    hasNetworkScoreData,
    lastScanTelemetry,
    lastScanAt,
  } = useScanContext();
  const { lanScanAllowed } = useNetworkConnectivity();
  const { names: customNames, saveName: saveCustomName } = useCustomNames();
  const { message: toastMessage, showToast } = useToast();
  const { lang, isRtl } = useLanguage();
  const t = useCallback((key: keyof typeof UI): string => (UI[key] as any)[lang] || UI[key].en, [lang]);
  const [networkInfo, setNetworkInfo] = useState<NetworkInfo | null>(null);
  const [apiNetwork, setApiNetwork] = useState<{ssid: string | null; gateway: string | null} | null>(null);
  const [viewMode, setViewMode] = useState<DevicesViewMode>("list");
  const [selectedDevice, setSelectedDevice] = useState<DeviceRow | null>(null);
  const [aliasingDevice, setAliasingDevice] = useState<DeviceRow | null>(null);
  const [aliasName, setAliasName] = useState("");
  const selectedIp = selectedDevice?.ip ?? null;

  const handleSaveAlias = useCallback(async () => {
    if (!aliasingDevice) return;
    try {
      await fetch("/api/devices/alias", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          ipAddress: aliasingDevice.ip,
          aliasName: aliasName.trim(),
        }),
      });
      
      saveCustomName(aliasingDevice.ip, aliasName.trim());
      patchDevice(aliasingDevice.ip, {
        displayName: aliasName.trim() || null,
        customName: aliasName.trim() || null,
      });
      
      showToast(t("save"));
      setAliasingDevice(null);
    } catch (e) {
      showToast("Failed to save alias");
    }
  }, [aliasingDevice, aliasName, patchDevice, saveCustomName, showToast, t]);

  useEffect(() => {
    invoke<NetworkInfo>("get_network_info")
      .then(setNetworkInfo)
      .catch(() => { /* non-fatal */ });
  }, []);

  useEffect(() => {
    invoke<Array<{ssid: string | null; gateway: string | null}>>("get_networks")
      .then((data) => {
        if (Array.isArray(data) && data.length > 0) {
          setApiNetwork({ ssid: data[0].ssid ?? null, gateway: data[0].gateway ?? null });
        }
      })
      .catch(() => {});
  }, []);

  const handleCopyIp = useCallback(async (ip: string) => {
    try {
      await navigator.clipboard.writeText(ip);
      showToast(t("copyIpAddress"));
    } catch {
      showToast(t("copyFailed"));
    }
  }, [showToast, t]);

  const [wakingDeviceIp, setWakingDeviceIp] = useState<string | null>(null);

  const handleWakeDevice = useCallback(
    async (device: DeviceRow) => {
      if (!isTauri() || !isValidMacForWakeOnLan(device.mac)) {
        return;
      }
      setWakingDeviceIp(device.ip);
      try {
        const msg = await invoke<string>("wake_device", {
          macAddress: device.mac.trim(),
        });
        showToast(msg);
      } catch (e) {
        const text =
          e instanceof Error
            ? e.message
            : typeof e === "string"
              ? e
              : "Wake on LAN failed";
        showToast(text);
      } finally {
        setWakingDeviceIp(null);
      }
    },
    [showToast],
  );
  const [isPortScanRunning, setIsPortScanRunning] = useState(false);
  const [forensicAutoScanning, setForensicAutoScanning] = useState(false);
  const [forensicByIp, setForensicByIp] = useState<
    Record<string, DeviceForensicRecord>
  >({});
  const [portScanProgress, setPortScanProgress] = useState(0);
  const [openPorts, setOpenPorts] = useState<number[]>([]);
  const [portScanError, setPortScanError] = useState<string | null>(null);
  const [likelyType, setLikelyType] = useState<string | null>(null);
  const [interrogationName, setInterrogationName] = useState<string | null>(null);
  const [isDeepScanAllRunning, setIsDeepScanAllRunning] = useState(false);
  const [isScanStarting, setIsScanStarting] = useState(false);
  const [isScanButtonCooldown, setIsScanButtonCooldown] = useState(false);
  const [scanMode, setScanMode] = useState<ScanMode>("silent");
  const [activeScanMode, setActiveScanMode] = useState<ScanMode | null>(null);
  const [scanStartedAt, setScanStartedAt] = useState<number | null>(null);
  const [scanTickMs, setScanTickMs] = useState<number>(Date.now());
  const [pingSamples, setPingSamples] = useState<number[]>([]);
  const [deviceFilter, setDeviceFilter] = useState<"all" | "online" | "offline" | "new">("all");
  const [sortOrder, setSortOrder] = useState<"ip" | "name" | "recent">("ip");
  const [hideStale, setHideStale] = useState(true);
  const progressTimer = useRef<number | null>(null);
  const scanButtonCooldownTimer = useRef<number | null>(null);
  const isCommandCenterWide = useFoldableWideLayout();

  const networkOverview = useMemo(() => {
    const valid = devices.filter(
      (d) =>
        d != null &&
        typeof d.ip === "string" &&
        d.ip.trim() !== "",
    );
    const onlineCount = valid.filter((d) => d.isOnline).length;
    const totalKnown = valid.length;
    const gatewayDevice =
      valid.find((d) => d.ip === apiNetwork?.gateway) ??
      valid.find((d) => d.likelyType === "Router / Gateway") ??
      valid.find((d) => d.ip.endsWith(".1")) ??
      null;
    return { gatewayDevice, onlineCount, totalKnown };
  }, [devices, apiNetwork]);

  const SEVEN_DAYS_MS = 7 * 24 * 60 * 60 * 1000;

  const staleCount = useMemo(
    () =>
      devices.filter(
        (d) => !d.isOnline && d.lastSeen && Date.now() - d.lastSeen > SEVEN_DAYS_MS,
      ).length,
    [devices],
  );

  const filteredDevices = useMemo(() => {
    const valid = devices.filter(
      (d) => d != null && typeof d.ip === "string" && d.ip.trim() !== "",
    );
    const withoutStale = hideStale
      ? valid.filter(
          (d) =>
            d.isOnline ||
            !d.lastSeen ||
            Date.now() - d.lastSeen <= SEVEN_DAYS_MS,
        )
      : valid;
    let filtered: typeof withoutStale;
    if (deviceFilter === "online") filtered = withoutStale.filter((d) => d.isOnline);
    else if (deviceFilter === "offline") filtered = withoutStale.filter((d) => !d.isOnline);
    else if (deviceFilter === "new") filtered = withoutStale.filter((d) => d.isNew);
    else filtered = withoutStale;

    const copy = [...filtered];
    if (sortOrder === "name") {
      copy.sort((a, b) => {
        const nameA = (
          customNames[a.ip]?.trim() ||
          a.customName?.trim() ||
          a.displayName?.trim() ||
          a.interrogationName?.trim() ||
          a.hostname?.trim() ||
          a.mdnsHostname?.trim() ||
          a.name?.trim() ||
          a.ip
        ).toLowerCase();
        const nameB = (
          customNames[b.ip]?.trim() ||
          b.customName?.trim() ||
          b.displayName?.trim() ||
          b.interrogationName?.trim() ||
          b.hostname?.trim() ||
          b.mdnsHostname?.trim() ||
          b.name?.trim() ||
          b.ip
        ).toLowerCase();
        return nameA.localeCompare(nameB);
      });
    } else if (sortOrder === "recent") {
      copy.sort((a, b) => (b.lastSeen ?? 0) - (a.lastSeen ?? 0));
    } else {
      // IP sort — compare octets numerically
      copy.sort((a, b) => {
        const ao = a.ip.split(".").map(Number);
        const bo = b.ip.split(".").map(Number);
        for (let i = 0; i < 4; i++) {
          const diff = (ao[i] ?? 0) - (bo[i] ?? 0);
          if (diff !== 0) return diff;
        }
        return 0;
      });
    }
    return copy;
  }, [devices, deviceFilter, hideStale, sortOrder, customNames]);

  const getTrustScore = useCallback(
    (device: DeviceRow) =>
      displayTrustAfterForensic(device, forensicByIp[device.ip]),
    [forensicByIp],
  );

  const clearProgressTimer = () => {
    if (progressTimer.current !== null) {
      window.clearInterval(progressTimer.current);
      progressTimer.current = null;
    }
  };

  useEffect(() => {
    if (scanRuntimeError) {
      showToast(scanRuntimeError);
      clearScanRuntimeError();
    }
  }, [clearScanRuntimeError, scanRuntimeError, showToast]);

  useEffect(() => {
    if (isScanning) {
      setScanStartedAt((prev) => prev ?? Date.now());
      const id = window.setInterval(() => setScanTickMs(Date.now()), 1000);
      return () => window.clearInterval(id);
    }
    setScanStartedAt(null);
    setScanTickMs(Date.now());
    return undefined;
  }, [isScanning]);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    let cancelled = false;
    let unlistenDeepScanProgress: (() => void) | undefined;

    void (async () => {
      unlistenDeepScanProgress = await listen<DeepScanProgressPayload>(
        "deep-scan-progress",
        (event) => {
          if (cancelled) {
            return;
          }
          const { ip, openPorts: ports, likelyType: lt } = event.payload;
          const nextLikelyType = lt?.trim() ? lt : null;
          patchDevice(ip, { likelyType: nextLikelyType });
          setForensicByIp((prev) => ({
            ...prev,
            [ip]: { scanned: true, openPorts: ports ?? [] },
          }));

          if (selectedIp === ip) {
            setOpenPorts(ports ?? []);
            setLikelyType(nextLikelyType);
          }
        },
      );
    })();

    return () => {
      cancelled = true;
      unlistenDeepScanProgress?.();
    };
  }, [patchDevice, selectedIp]);

  useEffect(() => {
    return () => {
      if (progressTimer.current !== null) {
        window.clearInterval(progressTimer.current);
      }
      if (scanButtonCooldownTimer.current !== null) {
        window.clearTimeout(scanButtonCooldownTimer.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!selectedIp) {
      return;
    }
    const fresh = devices.find((d) => d.ip === selectedIp);
    if (!fresh) {
      setSelectedDevice(null);
      setOpenPorts([]);
      setPortScanProgress(0);
      setPortScanError(null);
      setLikelyType(null);
      setInterrogationName(null);
      clearProgressTimer();
      setIsPortScanRunning(false);
    } else {
      setSelectedDevice(fresh);
    }
  }, [devices, selectedIp]);

  useEffect(() => {
    if (!selectedDevice || selectedDevice.status !== "Online") {
      setPingSamples([]);
      return;
    }

    const { ip, mac } = selectedDevice;
    // Seed from session cache so samples survive panel close/reopen.
    setPingSamples(getLatencySamples(mac, ip));

    let unlisten: (() => void) | undefined;

    void (async () => {
      unlisten = await listen<{ ip: string; latencyMs: number | null }>(
        "latency_update",
        (event) => {
          if (event.payload.ip !== ip) return;
          const ms = event.payload.latencyMs;
          if (typeof ms === "number") {
            const next = addLatencySample(mac, ip, ms);
            setPingSamples([...next]);
          }
        },
      );
      try {
        await invoke("start_latency_stream", { ip });
      } catch {
        /* ignore — stream best-effort */
      }
    })();

    return () => {
      unlisten?.();
      void invoke("stop_latency_stream").catch(() => undefined);
    };
  }, [selectedDevice?.ip, selectedDevice?.status]);

  useEffect(() => {
    if (!selectedDevice || selectedDevice.status !== "Online") {
      setForensicAutoScanning(false);
      return;
    }
    const ip = selectedDevice.ip;

    // Skip the full re-scan when we already have a warm forensic result from a
    // previous Port Guardian run on this device.  The cached data was already
    // seeded into local state by onSelectDevice, so there is nothing to do.
    if (forensicByIp[ip]?.scanned) {
      setForensicAutoScanning(false);
      return;
    }

    let cancelled = false;
    setForensicAutoScanning(true);
    setPortScanError(null);
    void (async () => {
      try {
        const result = await invoke<PortScanPayload>("scan_device_ports", {
          ip,
        });
        if (cancelled) {
          return;
        }
        const ports = result.openPorts ?? [];
        const lt   = result.likelyType || null;
        const iname = result.interrogationName || null;
        console.log(
          "[SCAN_TRACE_TYPE] Auto Port Guardian resolved | IP:", ip,
          "openPorts:", ports,
          "raw likelyType:", result.likelyType ?? "(null/undefined)",
          "→ lt:", lt,
          "interrogationName:", iname,
        );
        setOpenPorts(ports);
        setLikelyType(lt);
        setInterrogationName(iname);
        setForensicByIp((prev) => ({
          ...prev,
          [ip]: { scanned: true, openPorts: ports },
        }));
        patchDevice(ip, { likelyType: lt, interrogationName: iname });
      } catch (error) {
        if (!cancelled) {
          setPortScanError(
            error instanceof Error
              ? error.message
              : "Port Guardian scan failed",
          );
        }
      } finally {
        if (!cancelled) {
          setForensicAutoScanning(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedDevice?.ip, selectedDevice?.status, forensicByIp, patchDevice]);

  const handleScanWithHaptics = useCallback(async () => {
    if (isScanning || isScanStarting || isScanButtonCooldown) {
      return;
    }
    setIsScanStarting(true);
    setIsScanButtonCooldown(true);
    if (scanButtonCooldownTimer.current !== null) {
      window.clearTimeout(scanButtonCooldownTimer.current);
    }
    scanButtonCooldownTimer.current = window.setTimeout(() => {
      setIsScanButtonCooldown(false);
      scanButtonCooldownTimer.current = null;
    }, 10_000);
    setActiveScanMode(scanMode);
    try {
      if (isTauri()) {
        try {
          await vibrate(10);
        } catch {
          /* Haptics may be unavailable on desktop / some devices. */
        }
      }
      await triggerScan(scanMode);
    } catch (e) {
      console.error(e);
    } finally {
      setIsScanStarting(false);
      setActiveScanMode(null);
      if (isTauri()) {
        try {
          await vibrate(10);
          await new Promise<void>((resolve) => {
            window.setTimeout(resolve, 50);
          });
          await vibrate(10);
        } catch {
          /* Ignore completion haptics failures. */
        }
      }
    }
  }, [isScanButtonCooldown, isScanStarting, isScanning, scanMode, triggerScan]);

  const scanCountLabel = useMemo(() => {
    if (!isScanning || totalHosts <= 0) {
      return null;
    }
    return t("scanningHostProgress")
      .replace("{done}", String(Math.min(scannedHosts, totalHosts)))
      .replace("{total}", String(totalHosts));
  }, [isScanning, scannedHosts, t, totalHosts]);

  const isLongRunningScan = useMemo(() => {
    if (!isScanning || scanStartedAt === null) {
      return false;
    }
    return scanTickMs - scanStartedAt > 40_000;
  }, [isScanning, scanStartedAt, scanTickMs]);
  const isSlowScan = useMemo(() => {
    if (!isScanning || scanStartedAt === null) {
      return false;
    }
    return scanTickMs - scanStartedAt > 20_000;
  }, [isScanning, scanStartedAt, scanTickMs]);

  const displayProgressPct = isLongRunningScan
    ? Math.max(progressPct, 95)
    : progressPct;
  const scanRemainingSeconds = useMemo(() => {
    if (!isScanning || scanStartedAt === null) {
      return null;
    }
    const elapsedMs = Math.max(0, scanTickMs - scanStartedAt);
    return Math.max(0, Math.ceil((45_000 - elapsedMs) / 1000));
  }, [isScanning, scanStartedAt, scanTickMs]);
  const scanningTimeRemainingLabel = useMemo(() => {
    if (scanRemainingSeconds === null) {
      return null;
    }
    return t("scanningTimeRemaining").replace("{sec}", String(scanRemainingSeconds));
  }, [scanRemainingSeconds, t]);
  const scanActionLabel = scanMode === "silent" ? t("scanSilent") : t("scanAggressive");
  const scanButtonClassName = "shrink-0 gap-2 bg-accent hover:bg-accent-hover text-white";
  const radarFabClassName = "bg-accent hover:bg-accent-hover";
  const progressMode = activeScanMode ?? scanMode;
  const progressModeLabel =
    lang === "ar"
      ? progressMode === "silent"
        ? arStrings.devices.silentModeBadge
        : arStrings.devices.aggressiveModeBadge
      : progressMode === "silent"
        ? t("silentModeBadge")
        : t("aggressiveModeBadge");

  // ── Hardware back button interception ────────────────────────────────────────
  // Push a dummy history entry whenever the user enters a sub-state (device
  // inspector open, or star-map view).  The Android hardware back button fires
  // a `popstate` event via the HashRouter's history stack; we intercept it here
  // before React Router sees it so we can dismiss the sub-state instead of
  // navigating away from /devices and wiping the scan results.
  useEffect(() => {
    const onPop = () => {
      if (selectedDevice !== null) {
        // Close inspector — re-push so the route stays at /devices.
        window.history.pushState(null, "", window.location.href);
        setSelectedDevice(null);
        setPortScanError(null);
        setOpenPorts([]);
        setPortScanProgress(0);
        setLikelyType(null);
        setInterrogationName(null);
        setIsPortScanRunning(false);
      } else if (viewMode === "starmap") {
        window.history.pushState(null, "", window.location.href);
        setViewMode("list");
      }
      // Otherwise let React Router handle the pop normally.
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, [selectedDevice, viewMode]);

  const onSelectDevice = (device: DeviceRow) => {
    // Push a history entry so Android back closes the inspector rather than
    // navigating away from the devices page.
    window.history.pushState(null, "", window.location.href);
    setSelectedDevice(device);
    setPortScanProgress(0);
    setPortScanError(null);
    // Seed from previously patched device state so the inspector shows the last
    // Port Guardian result immediately instead of flashing "Unknown" while the
    // auto-scan re-runs.
    const cached = forensicByIp[device.ip];
    setOpenPorts(cached?.openPorts ?? []);
    setLikelyType(device.likelyType ?? null);
    setInterrogationName(device.interrogationName ?? null);
  };

  const runPortScan = async () => {
    if (!selectedDevice || isPortScanRunning || forensicAutoScanning) {
      return;
    }

    setIsPortScanRunning(true);
    setPortScanProgress(0);
    setOpenPorts([]);
    setPortScanError(null);
    setInterrogationName(null);
    clearProgressTimer();

    progressTimer.current = window.setInterval(() => {
      setPortScanProgress((prev) => (prev < 90 ? prev + 7 : prev));
    }, 120);

    try {
      const result = await invoke<PortScanPayload>("scan_device_ports", {
        ip: selectedDevice.ip,
      });
      const ports = result.openPorts ?? [];
      const lt    = result.likelyType || null;
      const iname = result.interrogationName || null;
      console.log(
        "[SCAN_TRACE_TYPE] Manual Port Guardian resolved | IP:", selectedDevice.ip,
        "openPorts:", ports,
        "raw likelyType:", result.likelyType ?? "(null/undefined)",
        "→ lt:", lt,
        "interrogationName:", iname,
      );
      setOpenPorts(ports);
      setLikelyType(lt);
      setInterrogationName(iname);
      setForensicByIp((prev) => ({
        ...prev,
        [selectedDevice.ip]: { scanned: true, openPorts: ports },
      }));
      patchDevice(selectedDevice.ip, { likelyType: lt, interrogationName: iname });
      setPortScanProgress(100);
    } catch (error) {
      setPortScanError(
        error instanceof Error ? error.message : "Port scan request failed",
      );
      setPortScanProgress(0);
    } finally {
      clearProgressTimer();
      setIsPortScanRunning(false);
    }
  };

  const runDeepScanAll = useCallback(async () => {
    if (isDeepScanAllRunning || !isTauri()) {
      return;
    }
    const ips = devices.map((device) => device.ip).filter(Boolean);
    if (ips.length === 0) {
      showToast(t("noDevicesFound"));
      return;
    }

    setIsDeepScanAllRunning(true);
    try {
      await invoke("scan_all_device_ports", { ips });
      showToast(t("deepScanDone"));
    } catch (error) {
      console.error(error);
      showToast(t("deepScanFailed"));
    } finally {
      setIsDeepScanAllRunning(false);
    }
  }, [devices, isDeepScanAllRunning, showToast, t]);

  // "last scan Xm ago" label for browser mode network header
  const lastScanAgoStr = useMemo(() => {
    const ref = lastScanAt ?? (() => {
      const maxTs = Math.max(...devices.map((d) => d.lastSeen ?? 0).filter((t) => t > 0));
      return maxTs > 0 ? new Date(maxTs) : null;
    })();
    if (!ref) return null;
    const secs = Math.floor((Date.now() - ref.getTime()) / 1000);
    if (secs < 90) return "just now";
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m ago`;
    return `${Math.floor(mins / 60)}h ago`;
  }, [lastScanAt, devices]);

  // Subnet derived from gateway IP (e.g. "192.168.254.1" → "192.168.254.0/24")
  const subnetCidr = useMemo(() => {
    const gw = apiNetwork?.gateway ?? null;
    if (!gw) return null;
    const parts = gw.split(".");
    if (parts.length !== 4) return null;
    return `${parts[0]}.${parts[1]}.${parts[2]}.0/24`;
  }, [apiNetwork]);

  const isStarMap = viewMode === "starmap";

  return (
    <div
      dir={isRtl ? "rtl" : "ltr"}
      className={cn(
        "relative pb-36 md:pb-8",
        isStarMap && "flex min-h-0 flex-1 flex-col",
      )}
    >
      <header className="flex shrink-0 flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h2 className="text-3xl font-semibold text-primary">{t("pageTitle")}</h2>
          <p className="mt-1 text-sm text-secondary">
            {t("pageDesc")}
          </p>
        </div>
        <div
          className="inline-flex rounded-lg border border-separator bg-surface-alt p-0.5"
          role="group"
          aria-label="Devices view"
        >
          <button
            type="button"
            onClick={() => setViewMode("list")}
            className={cn(
              "rounded-md px-3 py-1.5 text-xs font-semibold transition-colors",
              viewMode === "list"
                ? "bg-surface text-primary"
                : "text-secondary",
            )}
          >
            {t("listView")}
          </button>
          <button
            type="button"
            onClick={() => { window.history.pushState(null, "", window.location.href); setViewMode("starmap"); }}
            className={cn(
              "rounded-md px-3 py-1.5 text-xs font-semibold transition-colors",
              viewMode === "starmap"
                ? "bg-surface text-primary"
                : "text-secondary",
            )}
          >
            {t("starMapView")}
          </button>
        </div>
      </header>

      <NetworkHUD info={networkInfo} />

      {scanPermissionError ? (
        <div
          role="alert"
          className="mt-4 flex items-start justify-between gap-3 rounded-xl border border-amber-500/35 bg-amber-500/10 px-4 py-3 text-sm text-amber-100"
        >
          <span>{scanPermissionError}</span>
          <button
            type="button"
            onClick={() => clearScanPermissionError()}
            className="shrink-0 rounded-md border border-amber-500/40 px-2 py-1 text-xs font-medium text-amber-200 hover:bg-amber-500/20"
          >
            {t("dismiss")}
          </button>
        </div>
      ) : null}

      {isScanning ? (
        <div
          className="relative mt-3 space-y-3 overflow-hidden rounded-xl border border-separator bg-surface p-3"
        >
          <div className="flex items-center justify-between text-xs text-primary">
            <div className="flex min-w-0 items-center gap-2.5">
              <div className="relative flex h-2.5 w-2.5 shrink-0">
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent/50" />
                <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-accent" />
              </div>
              <span className="min-w-0 font-medium text-primary">
                {isLongRunningScan
                  ? `${t("finalizing")} ${displayProgressPct}%`
                  : `${t("scanning")}... ${displayProgressPct}%`}
              </span>
              <span
                className={cn(
                  "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-semibold",
                  progressMode === "silent"
                    ? "border-accent/40 bg-accent-muted text-primary"
                    : "border-warning/40 bg-warning/20 text-warning",
                )}
              >
                {progressMode === "silent" ? (
                  <ShieldCheck className="size-3" aria-hidden />
                ) : (
                  <Zap className="size-3" aria-hidden />
                )}
                {progressModeLabel}
              </span>
            </div>
            {scanCountLabel ? (
              <span className="shrink-0 font-mono text-secondary">{scanCountLabel}</span>
            ) : null}
          </div>
          <div
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={displayProgressPct}
            aria-label="Network scan progress"
            className="h-1.5 w-full shrink-0 overflow-hidden rounded-full bg-surface-alt"
          >
            <div
              className="h-full rounded-full bg-accent transition-[width] duration-300 ease-out"
              style={{ width: `${displayProgressPct}%` }}
            />
          </div>
          <p className="text-[11px] text-secondary">
            {t("largeNetworkHint")}
          </p>
        </div>
      ) : null}

      {isStarMap ? (
        <div className="mt-6 flex min-h-0 flex-1 flex-col">
          <div className="relative min-h-[min(72dvh,calc(100dvh-12rem))] flex-1 overflow-hidden rounded-2xl border border-separator bg-void">
            <NetworkStarMap
              averageLatencyMs={averageLatencyMs}
              getTrustScore={getTrustScore}
              selectedIp={selectedIp}
              onDeviceClick={(ip) => {
                const target = devices.find((d) => d.ip === ip);
                if (target) {
                  onSelectDevice(target);
                }
              }}
            />
          </div>
        </div>
      ) : (
      <div
        className={cn(
          "flex min-h-0 flex-col gap-4 overflow-hidden lg:h-[min(85vh,720px)] lg:max-h-[min(85vh,720px)] lg:flex-row",
          isScanning ? "mt-5" : "mt-6",
        )}
        role={isCommandCenterWide ? "region" : undefined}
        aria-label={isCommandCenterWide ? "Master-detail devices" : undefined}
      >
        {/* Left: full-width list on phone / cover; 1/3 on lg+ with split detail pane */}
        <div
          className={cn(
            "flex min-h-0 w-full flex-col overflow-hidden transition-all duration-300",
            selectedDevice ? "lg:w-1/3 lg:max-w-none lg:shrink-0" : "lg:w-full",
          )}
        >
          <Card className="flex min-h-0 flex-1 flex-col ring-0">
            <CardHeader className="shrink-0">
              <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
                <CardTitle className="text-primary">{t("networkDevices")}</CardTitle>
                <div className="flex flex-wrap items-center gap-2">
                  <div
                    className="inline-flex rounded-lg border border-separator bg-surface-alt p-0.5"
                    role="group"
                    aria-label={t("scanModeLabel")}
                  >
                    <button
                      type="button"
                      onClick={() => setScanMode("silent")}
                      className={cn(
                        "rounded-md px-3 py-1.5 text-xs font-semibold transition-colors",
                        scanMode === "silent"
                          ? "bg-surface text-primary"
                          : "text-secondary hover:text-primary",
                      )}
                    >
                      {t("silentMode")}
                    </button>
                    <button
                      type="button"
                      onClick={() => setScanMode("aggressive")}
                      className={cn(
                        "rounded-md px-3 py-1.5 text-xs font-semibold transition-colors",
                        scanMode === "aggressive"
                          ? "bg-surface text-primary"
                          : "text-secondary hover:text-primary",
                      )}
                    >
                      {t("aggressiveMode")}
                    </button>
                  </div>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={runDeepScanAll}
                    disabled={isScanning || isDeepScanAllRunning || devices.length === 0}
                    className="gap-2 text-secondary hover:text-primary hover:bg-surface-alt"
                  >
                    {isDeepScanAllRunning ? (
                      <>
                        <Loader2 className="size-4 animate-spin" aria-hidden />
                        {t("deepScanningAll")}
                      </>
                    ) : (
                      <>
                        <ScanSearch className="size-4" aria-hidden />
                        {t("deepScanAll")}
                      </>
                    )}
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    onClick={handleScanWithHaptics}
                    disabled={isScanning || isScanStarting || isScanButtonCooldown}
                    className={scanButtonClassName}
                  >
                    {isScanning ? (
                      <>
                        <Loader2 className="size-4 animate-spin" aria-hidden />
                        {t("scanningSimple")}
                      </>
                    ) : (
                      scanActionLabel
                    )}
                  </Button>
                </div>
              </div>
              <CardDescription className="mt-1 text-secondary">
                {t("tapRadar")}
              </CardDescription>
            </CardHeader>
            <CardContent className="min-h-0 flex-1 overflow-y-auto lg:pt-0">
              <div
                className={cn(
                  "px-4 pt-1 lg:px-2",
                  isRtl && "text-right",
                )}
              >
                {networkOverview.totalKnown === 0 ? (
                  <div
                    className="mb-6 flex flex-col items-center justify-center gap-3 rounded-xl bg-surface p-6 text-center"
                    role="region"
                    aria-label={t("readyToScan")}
                  >
                    <div className="flex size-12 items-center justify-center rounded-xl bg-surface-alt text-secondary">
                      <Network className="size-7" aria-hidden />
                    </div>
                    <p className="text-[15px] font-semibold text-primary">
                      {t("readyToScan")}
                    </p>
                    <p className="max-w-sm text-sm leading-relaxed text-secondary">
                      {t("readyToScanHint")}
                    </p>
                  </div>
                ) : (
                  <div
                    className="mb-6 rounded-xl bg-surface p-6"
                    role="region"
                    aria-label={t("currentNetworkKicker")}
                  >
                    <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                      <div className="min-w-0">
                        {/* Kicker label — hidden in browser mode, context is obvious from sidebar */}
                        {isTauri() && (
                          <p className="text-[10px] font-semibold uppercase tracking-[0.2em] text-secondary">
                            {t("currentNetworkKicker")}
                          </p>
                        )}
                        <h2 className={cn(
                          "break-words font-bold leading-tight text-primary",
                          isTauri() ? "mt-2 text-2xl sm:text-3xl" : "text-2xl sm:text-3xl",
                        )}>
                          {networkOverview.gatewayDevice
                            ? (customNames[networkOverview.gatewayDevice.ip]?.trim() ||
                               networkOverview.gatewayDevice.customName?.trim() ||
                               networkOverview.gatewayDevice.displayName?.trim() ||
                               t("gatewayWithIp").replace("{ip}", networkOverview.gatewayDevice.ip))
                            : apiNetwork?.ssid
                              ? (apiNetwork.ssid === "Home" ? "Home Network" : apiNetwork.ssid)
                              : t("thisNetworkTitle")}
                        </h2>
                        {!isTauri() ? (
                          <p className="mt-1.5 text-sm text-secondary">
                            {[
                              subnetCidr,
                              `${networkOverview.totalKnown} known devices`,
                              lastScanAgoStr ? `Last scan ${lastScanAgoStr}` : null,
                            ].filter(Boolean).join(" · ")}
                          </p>
                        ) : (
                          <p className="mt-1.5 text-sm font-medium text-secondary">
                            {apiNetwork?.gateway
                              ? t("gatewayWithIp").replace("{ip}", apiNetwork.gateway)
                              : networkOverview.gatewayDevice
                                ? t("gatewayWithIp").replace(
                                    "{ip}",
                                    networkOverview.gatewayDevice.ip,
                                  )
                                : t("noGatewayLine")}
                          </p>
                        )}
                      </div>
                      <div
                        className={cn(
                          "shrink-0",
                          isRtl && "self-end sm:self-start",
                        )}
                      >
                        <span className="inline-flex items-center rounded-full border border-accent/35 bg-accent-muted px-3.5 py-1.5 text-xs font-bold tabular-nums text-accent">
                          {t("onlineVsTotalPill")
                            .replace(
                              "{online}",
                              String(networkOverview.onlineCount),
                            )
                            .replace(
                              "{total}",
                              String(networkOverview.totalKnown),
                            )}
                        </span>
                      </div>
                    </div>
                  </div>
                )}
              </div>
              {devices.length === 0 ? (
                <div className="mx-4 rounded-xl border border-dashed border-separator bg-surface px-4 py-12 text-center text-[13px] text-secondary">
                  {isScanning
                    ? t("scanningNetworkProgress").replace("{pct}", String(progressPct))
                    : t("tapRadar")}
                </div>
              ) : (
                <>
                  {/* Filter pills — Section 5.4 */}
                  <div className="flex flex-wrap items-center justify-between gap-1.5 px-4 py-2">
                    <div className="flex flex-wrap items-center gap-1.5">
                      <div
                        className="flex gap-1.5 overflow-x-auto"
                        role="group"
                        aria-label="Filter devices"
                      >
                        {(
                          [
                            { key: "all",     label: `${t("filterAll")} (${networkOverview.totalKnown})` },
                            { key: "online",  label: `${t("filterOnline")} (${networkOverview.onlineCount})` },
                            { key: "offline", label: `${t("filterOffline")} (${networkOverview.totalKnown - networkOverview.onlineCount})` },
                            { key: "new",     label: `${t("filterNew")} (${devices.filter((d) => d.isNew).length})` },
                          ] as const
                        ).map(({ key, label }) => (
                          <button
                            key={key}
                            type="button"
                            onClick={() => setDeviceFilter(key)}
                            className={cn(
                              "shrink-0 rounded-full px-3 py-1.5 text-[13px] font-medium transition-colors duration-150",
                              deviceFilter === key
                                ? "bg-accent text-white"
                                : "text-secondary hover:text-primary",
                            )}
                          >
                            {label}
                          </button>
                        ))}
                      </div>
                      {staleCount > 0 && (
                        <button
                          type="button"
                          onClick={() => setHideStale((v) => !v)}
                          className={cn(
                            "shrink-0 rounded-full border px-3 py-1.5 text-[12px] font-medium transition-colors duration-150",
                            hideStale
                              ? "border-separator text-tertiary hover:text-secondary"
                              : "border-accent/40 bg-accent-muted text-accent",
                          )}
                          title={hideStale ? "Stale devices are hidden — click to show" : "Click to hide devices offline > 7 days"}
                        >
                          {hideStale
                            ? t("showStaleLabel").replace("{n}", String(staleCount))
                            : t("hideStaleLabel")}
                        </button>
                      )}
                    </div>
                    {/* Sort control */}
                    <div
                      className="inline-flex shrink-0 rounded-lg border border-separator bg-surface-alt p-0.5"
                      role="group"
                      aria-label="Sort devices"
                    >
                      {(
                        [
                          { key: "ip",     label: "IP ↑" },
                          { key: "name",   label: "Name A-Z" },
                          { key: "recent", label: "Recent" },
                        ] as const
                      ).map(({ key, label }) => (
                        <button
                          key={key}
                          type="button"
                          onClick={() => setSortOrder(key)}
                          className={cn(
                            "rounded-md px-2.5 py-1 text-xs font-semibold transition-colors duration-150",
                            sortOrder === key
                              ? "bg-surface text-primary"
                              : "text-secondary hover:text-primary",
                          )}
                        >
                          {label}
                        </button>
                      ))}
                    </div>
                  </div>

                  {/* Grouped device card — Section 5.2 / 4 */}
                  <div className="px-4 pb-2 lg:px-2">
                    {isLoading && filteredDevices.length === 0 ? (
                      <div className="flex flex-col items-center justify-center py-12 text-secondary">
                        <Loader2 className="mb-3 h-8 w-8 animate-spin text-accent" />
                        <p className="text-sm font-medium tracking-tight">Syncing network nodes...</p>
                      </div>
                    ) : filteredDevices.length === 0 ? (
                      <div className="rounded-xl border border-dashed border-separator py-8 text-center text-[13px] text-secondary">
                        {t("filterAll")}
                      </div>
                    ) : (
                      <div className="bg-surface rounded-xl overflow-hidden">
                        {filteredDevices.map((device, idx) => {
                          const customLabel = customNames[device.ip]?.trim();
                          const { primary, primaryClassName, prominent } =
                            getDeviceListPrimaryLine(
                              device,
                              customLabel,
                              t("unknownDevice"),
                            );
                          const metaLine = getDeviceListMetaLine(device);
                          const forensicRec = forensicByIp[device.ip];
                          const effectiveTrust = displayTrustAfterForensic(
                            device,
                            forensicRec,
                          );
                          const criticalCard =
                            forensicRec?.scanned &&
                            forensicCriticalPorts(forensicRec.openPorts);
                          const scoreGlow = effectiveTrust >= 75 && !criticalCard;
                          const isSelected = selectedDevice?.ip === device.ip;

                          return (
                            <Fragment key={rowKey(device)}>
                              {idx > 0 && (
                                <div className="h-px bg-separator ml-[52px]" aria-hidden />
                              )}
                              <DeviceRowShell
                                device={device}
                                isSelected={isSelected}
                                scoreGlow={scoreGlow}
                                onClick={() => onSelectDevice(device)}
                                onKeyDown={(e) => {
                                  if (e.key === "Enter" || e.key === " ") {
                                    e.preventDefault();
                                    onSelectDevice(device);
                                  }
                                }}
                              >
                                <DeviceStatusDot device={device} />
                                <DeviceTypeIcon device={device} />
                                <DeviceListNameHeading
                                  primary={primary}
                                  secondary={metaLine}
                                  prominent={prominent}
                                  primaryClassName={primaryClassName}
                                >
                                  {!device.isOnline && (
                                    <div className="flex items-center gap-1">
                                      <DeviceLastSeenLabel lastSeen={device.lastSeen} />
                                      <DeviceOfflineBadge lastSeen={device.lastSeen} />
                                    </div>
                                  )}
                                  {isTauri() && !device.isOnline && isValidMacForWakeOnLan(device.mac) ? (
                                    <DeviceListWakeButton
                                      isSending={wakingDeviceIp === device.ip}
                                      label={t("wakeDeviceList")}
                                      sendingLabel={t("wakeDeviceSending")}
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        void handleWakeDevice(device);
                                      }}
                                    />
                                  ) : null}
                                  {device.isOnline && (
                                    (device.likelyType?.includes("Router") || 
                                     device.likelyType?.includes("Gateway") || 
                                     device.ip.endsWith(".1") || 
                                     device.ip.endsWith(".2"))
                                  ) && (
                                    <DeviceLaunchPortalLink 
                                      ip={device.ip} 
                                      label={t("launchPortal")} 
                                    />
                                  )}
                                </DeviceListNameHeading>
                                <div className="flex shrink-0 items-center gap-1.5">
                                  <DeviceListEditButton
                                    title={t("editName")}
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      setAliasingDevice(device);
                                      setAliasName(customLabel || device.displayName || "");
                                    }}
                                  />
                                  {device.shieldHighlight ? (
                                    <Shield
                                      className="w-4 h-4 text-warning"
                                      aria-label="SSH or HTTP port open on last scan"
                                    />
                                  ) : null}
                                  <ChevronRight className="w-4 h-4 text-tertiary" aria-hidden />
                                </div>
                              </DeviceRowShell>
                            </Fragment>
                          );
                        })}
                      </div>
                    )}
                  </div>

                  {/* Telemetry line — bottom, text-tertiary */}
                  {lastScanTelemetry && !isScanning && (
                    <div className="flex items-center gap-1.5 px-4 pb-3 pt-1 font-mono text-[11px] text-tertiary">
                      <span className={cn(
                        "rounded px-1 py-0.5",
                        lastScanTelemetry.status === "failed"
                          ? "text-error"
                          : "text-tertiary",
                      )}>
                        {lastScanTelemetry.scanId}
                      </span>
                      {lastScanTelemetry.status === "failed" ? (
                        <span className="text-error">
                          failed — {lastScanTelemetry.failureReason ?? "unknown"}
                        </span>
                      ) : (
                        <>
                          <span>{lastScanTelemetry.batches}b</span>
                          <span>·</span>
                          <span>{lastScanTelemetry.ipcDevices} via IPC</span>
                          {lastScanTelemetry.staleBatches > 0 && (
                            <span className="text-warning">
                              {lastScanTelemetry.staleBatches} stale
                            </span>
                          )}
                        </>
                      )}
                    </div>
                  )}
                </>
              )}
            </CardContent>
          </Card>
        </div>

        {/* Right: 2/3 detail dashboard (lg+); mobile uses sheet over the list */}
        {selectedDevice ? (
          <div
            aria-label="Device command center"
            className="animate-in fade-in slide-in-from-right-4 duration-500 hidden min-h-0 w-full min-w-0 flex-col overflow-hidden rounded-xl bg-surface p-6 lg:flex lg:w-2/3"
          >
            <header className="mb-4 shrink-0 border-b border-separator pb-4">
              <h2 className="text-[15px] font-semibold text-primary">
                Device Command
              </h2>
              <p className="mt-1 text-xs uppercase tracking-widest text-secondary">
                Security posture — Port Guardian and exposure
              </p>
            </header>
            <CommandCenterHealthShield
              score={networkScore}
              hasData={hasNetworkScoreData}
              scanActive={isScanning}
            />
            <div className="min-h-0 flex-1 space-y-6 overflow-y-auto">
              <DeviceDetailsPanel
                key={selectedDevice.ip}
                device={selectedDevice}
                customName={customNames[selectedDevice.ip]}
                interrogationName={interrogationName}
                fingerprintLikelyType={likelyType ?? selectedDevice.likelyType ?? null}
                onSaveCustomName={async (name) => {
                  saveCustomName(selectedDevice.ip, name);
                  patchDevice(selectedDevice.ip, {
                    customName: name.trim() || null,
                  });
                  // Persist to server
                  await fetch(`/api/devices/${selectedDevice.mac}`, {
                    method: "PATCH",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ custom_name: name.trim() || null }),
                  });
                }}
                onSaveCustomIcon={async (url) => {
                  patchDevice(selectedDevice.ip, {
                    customIcon: url,
                  });
                  // Persist to server
                  await fetch(`/api/devices/${selectedDevice.mac}`, {
                    method: "PATCH",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ custom_icon: url }),
                  });
                }}
                onClose={() => setSelectedDevice(null)}
                strings={{
                  deviceDetails: t("deviceDetailsDashboard"),
                  ipAddress: t("ipAddress"),
                  macAddress: t("macAddress"),
                  likelyType: t("likelyType"),
                  wakeOnLan: t("wakeOnLan"),
                  runDeepScan: t("runDeepScanAction"),
                  editName: t("editName"),
                  setCustomName: t("setCustomName"),
                  save: t("save"),
                  cancel: t("cancel"),
                  clear: t("clear"),
                  noCustomName: t("noCustomName"),
                  customNamePlaceholder: t("customNamePlaceholder"),
                  custom: t("custom"),
                  notAvailable: t("fingerprintNotAvailable"),
                  wakeOnLanNeedMac: t("wakeOnLanNeedMac"),
                  deepScanNeedIp: t("deepScanNeedIp"),
                  deepScanPortsProgress: t("deepScanPortsProgress"),
                  deepScanOpenPortsTitle: t("deepScanOpenPortsTitle"),
                }}
                showUserMessage={showToast}
              />
              <DeviceInspector
                device={selectedDevice}
                pingSamples={pingSamples}
                isPortScanRunning={isPortScanRunning}
                forensicAutoScanning={forensicAutoScanning}
                portScanProgress={portScanProgress}
                openPorts={openPorts}
                portScanError={portScanError}
                likelyType={likelyType}
                interrogationName={interrogationName}
                onRunPortScan={runPortScan}
                customName={customNames[selectedDevice.ip]}
                onSaveCustomName={(name) => { saveCustomName(selectedDevice.ip, name); patchDevice(selectedDevice.ip, { customName: name.trim() || null }); }}
                onCopyIp={handleCopyIp}
                lang={lang}
              />
            </div>
          </div>
        ) : null}
      </div>
      )}

      <Sheet
        open={selectedDevice !== null && (!isCommandCenterWide || viewMode === "starmap")}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedDevice(null);
            setPortScanError(null);
            setOpenPorts([]);
            setPortScanProgress(0);
            setLikelyType(null);
            setInterrogationName(null);
            clearProgressTimer();
            setIsPortScanRunning(false);
          }
        }}
      >
        <SheetContent
          className={cn(
            "max-h-[100dvh] overflow-y-auto pb-28 pt-20",
            "border-separator bg-surface text-primary",
            "w-full max-w-lg sm:max-w-md",
          )}
        >
          {selectedDevice ? (
            <div className="space-y-6">
              <DeviceDetailsPanel
                key={selectedDevice.ip}
                device={selectedDevice}
                customName={customNames[selectedDevice.ip]}
                interrogationName={interrogationName}
                fingerprintLikelyType={likelyType ?? selectedDevice.likelyType ?? null}
                onSaveCustomName={async (name) => {
                  saveCustomName(selectedDevice.ip, name);
                  patchDevice(selectedDevice.ip, {
                    customName: name.trim() || null,
                  });
                  // Persist to server
                  await fetch(`/api/devices/${selectedDevice.mac}`, {
                    method: "PATCH",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ custom_name: name.trim() || null }),
                  });
                }}
                onSaveCustomIcon={async (url) => {
                  patchDevice(selectedDevice.ip, {
                    customIcon: url,
                  });
                  // Persist to server
                  await fetch(`/api/devices/${selectedDevice.mac}`, {
                    method: "PATCH",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ custom_icon: url }),
                  });
                }}
                onClose={() => setSelectedDevice(null)}
                strings={{
                  deviceDetails: t("deviceDetailsDashboard"),
                  ipAddress: t("ipAddress"),
                  macAddress: t("macAddress"),
                  likelyType: t("likelyType"),
                  wakeOnLan: t("wakeOnLan"),
                  runDeepScan: t("runDeepScanAction"),
                  editName: t("editName"),
                  setCustomName: t("setCustomName"),
                  save: t("save"),
                  cancel: t("cancel"),
                  clear: t("clear"),
                  noCustomName: t("noCustomName"),
                  customNamePlaceholder: t("customNamePlaceholder"),
                  custom: t("custom"),
                  notAvailable: t("fingerprintNotAvailable"),
                  wakeOnLanNeedMac: t("wakeOnLanNeedMac"),
                  deepScanNeedIp: t("deepScanNeedIp"),
                  deepScanPortsProgress: t("deepScanPortsProgress"),
                  deepScanOpenPortsTitle: t("deepScanOpenPortsTitle"),
                }}
                showUserMessage={showToast}
              />
              <DeviceInspector
                device={selectedDevice}
                pingSamples={pingSamples}
                isPortScanRunning={isPortScanRunning}
                forensicAutoScanning={forensicAutoScanning}
                portScanProgress={portScanProgress}
                openPorts={openPorts}
                portScanError={portScanError}
                likelyType={likelyType}
                interrogationName={interrogationName}
                onRunPortScan={runPortScan}
                customName={customNames[selectedDevice.ip]}
                onSaveCustomName={(name) => { saveCustomName(selectedDevice.ip, name); patchDevice(selectedDevice.ip, { customName: name.trim() || null }); }}
                onCopyIp={handleCopyIp}
                lang={lang}
                showSheetChrome
              />
            </div>
          ) : null}
        </SheetContent>
      </Sheet>

      <div className="fixed bottom-[calc(5.5rem+env(safe-area-inset-bottom))] right-5 z-[45] flex flex-col items-end gap-2 lg:hidden">
        <div className="inline-flex rounded-full border border-separator bg-surface p-0.5 shadow-xl">
          <button
            type="button"
            onClick={() => setScanMode("silent")}
            className={cn(
              "rounded-full px-3 py-1 text-[10px] font-semibold transition-colors duration-150",
              scanMode === "silent"
                ? "bg-accent text-white"
                : "text-secondary hover:text-primary",
            )}
          >
            {t("silentMode")}
          </button>
          <button
            type="button"
            onClick={() => setScanMode("aggressive")}
            className={cn(
              "rounded-full px-3 py-1 text-[10px] font-semibold transition-colors duration-150",
              scanMode === "aggressive"
                ? "bg-accent text-white"
                : "text-secondary hover:text-primary",
            )}
          >
            {t("aggressiveMode")}
          </button>
        </div>
        {isScanning ? (
          <span className="inline-flex items-center gap-2 rounded-full border border-separator bg-surface px-3 py-2 text-xs font-semibold text-primary">
            <span className="relative flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent/50" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-accent" />
            </span>
            <Loader2 className="size-4 shrink-0 animate-spin" aria-hidden />
            {scanningTimeRemainingLabel ?? t("scanning")}
          </span>
        ) : null}
        {isSlowScan ? (
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => {
              void cancelScan();
            }}
            className="h-8 rounded-full"
          >
            {t("cancelScan")}
          </Button>
        ) : null}
        <button
          type="button"
          onClick={() => {
            if (isScanning) {
              void cancelScan();
              return;
            }
            void handleScanWithHaptics();
          }}
          disabled={
            (!isScanning && (isScanStarting || isScanButtonCooldown)) ||
            (!isScanning && !lanScanAllowed)
          }
          title={
            isScanning
              ? t("cancelScan")
              : lanScanAllowed
              ? undefined
              : "Connect to Wi‑Fi to run a local network scan."
          }
          aria-label={
            !lanScanAllowed
              ? "Local scan requires Wi‑Fi"
              : isScanning
                ? "Scanning network"
                : "Scan network"
          }
          className={cn(
            "flex items-center justify-center border-4 border-void text-white transition-transform",
            "h-10 rounded-full px-3",
            (!isScanning && (!lanScanAllowed || isScanStarting))
              ? "cursor-not-allowed opacity-45"
              : "active:scale-95",
            isScanning
              ? "bg-error hover:bg-error/80"
              : !lanScanAllowed
                ? "bg-surface-alt"
                : radarFabClassName,
          )}
        >
          {isScanning ? (
            <span className="text-[11px] font-semibold">{t("cancelScan")}</span>
          ) : (
            <span className="inline-flex items-center gap-1.5 text-[11px] font-semibold">
              <Radar className="size-3.5" aria-hidden />
              {scanActionLabel}
            </span>
          )}
        </button>
      </div>
      <ToastNotification message={toastMessage} />

      {aliasingDevice && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm">
          <div className="w-full max-w-sm rounded-2xl bg-surface p-6 shadow-xl border border-separator animate-in zoom-in-95 duration-200">
            <h3 className="text-lg font-semibold text-primary">{t("editName")}</h3>
            <p className="mt-1 text-sm text-secondary">{aliasingDevice.ip}</p>
            <div className="mt-4">
              <input
                type="text"
                value={aliasName}
                onChange={(e) => setAliasName(e.target.value)}
                placeholder={t("customNamePlaceholder")}
                className="w-full rounded-xl border border-separator bg-surface-alt px-4 py-3 text-primary focus:outline-none focus:ring-2 focus:ring-accent"
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSaveAlias();
                  if (e.key === "Escape") setAliasingDevice(null);
                }}
              />
            </div>
            <div className="mt-6 flex gap-3">
              <Button
                variant="ghost"
                className="flex-1"
                onClick={() => setAliasingDevice(null)}
              >
                {t("cancel")}
              </Button>
              <Button
                className="flex-1 bg-accent text-white hover:bg-accent-hover"
                onClick={handleSaveAlias}
              >
                {t("save")}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}



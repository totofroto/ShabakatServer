import type { LucideIcon } from "lucide-react";
import {
  ArrowLeft,
  Camera,
  Cast,
  HardDrive,
  History as HistoryIcon,
  Home,
  Laptop,
  Lightbulb,
  Loader2,
  Monitor,
  Printer,
  Router,
  ShieldAlert,
  Smartphone,
  Speaker,
  Terminal,
  Tv,
} from "lucide-react";
import { listen, transport, invoke } from "@/lib/transport";
import { useEffect, useRef, useState } from "react";
import type { DeviceRow } from "@/hooks/useNetworkScan";
import { cn } from "@/lib/utils";
import { useLanguage } from "@/context/LanguageContext";

export type DiscoveredDevice = DeviceRow;

export type DeviceDetailsPanelStrings = {
  deviceDetails: string;
  ipAddress: string;
  macAddress: string;
  likelyType: string;
  wakeOnLan: string;
  runDeepScan: string;
  editName: string;
  setCustomName: string;
  save: string;
  cancel: string;
  clear: string;
  noCustomName: string;
  customNamePlaceholder: string;
  custom: string;
  notAvailable: string;
  wakeOnLanNeedMac: string;
  deepScanNeedIp: string;
  deepScanPortsProgress: string;
  deepScanOpenPortsTitle: string;
};

type DeepPortScanProgressPayload = {
  ip: string;
  totalPorts: number;
  portsChecked: number;
  openPorts: number[];
};

const DEEP_PORT_LABELS: Record<number, string> = {
  20: "FTP Data",
  21: "FTP",
  22: "SSH",
  23: "Telnet",
  25: "SMTP",
  53: "DNS",
  80: "HTTP",
  110: "POP3",
  135: "RPC",
  139: "NetBIOS",
  143: "IMAP",
  443: "HTTPS",
  445: "SMB",
  465: "SMTPS",
  587: "Submission",
  993: "IMAPS",
  995: "POP3S",
  3306: "MySQL",
  3389: "RDP",
  5000: "UPnP / alt-HTTP",
  5432: "PostgreSQL",
  6379: "Redis",
  8080: "HTTP Alt",
  8443: "HTTPS Alt",
  9200: "Elasticsearch",
  27017: "MongoDB",
};

const MDNS_SERVICE_LABELS: Record<string, string> = {
  googlecast: "Google Cast",
  hap: "HomeKit",
  printer: "Network Printer",
  spotify: "Spotify Connect",
  smb: "File Sharing (SMB)",
};

/** IPv4 only — matches typical LAN host rows from the scanner. */
export function isValidDeepScanIp(ip: string): boolean {
  return /^((25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(25[0-5]|2[0-4]\d|[01]?\d\d?)$/.test(ip.trim());
}

function hexDigitsFromMac(raw: string): string {
  return raw.replace(/[^0-9a-fA-F]/g, "");
}

export function formatMacForWolIfValid(raw: string): string | null {
  const hex = hexDigitsFromMac(raw);
  if (hex.length !== 12) return null;
  if (/^0{12}$/i.test(hex)) return null;
  const pairs = hex.match(/.{2}/g);
  if (!pairs) return null;
  return pairs.map((p) => p.toUpperCase()).join(":");
}

export function isWolEligibleDeviceMac(device: DeviceRow): boolean {
  const m = device.mac?.trim() ?? "";
  if (!m) return false;
  const low = m.toLowerCase();
  if (low === "unknown" || low === "mac restricted") return false;
  return formatMacForWolIfValid(m) !== null;
}

export function pickDeviceIcon(device: DeviceRow): LucideIcon {
  const type = (device.likelyType ?? device.deviceType ?? "").toLowerCase();
  const svc = (device.mdnsPrimaryService ?? "").toLowerCase();

  if (type.includes("router") || type.includes("gateway")) return Router;
  if (type.includes("nas") || type.includes("synology") || type.includes("qnap")) return HardDrive;
  if (type.includes("tv") || svc === "googlecast") return Tv;
  if (type.includes("print") || svc === "printer") return Printer;
  if (type.includes("mac") || type.includes("apple")) return Monitor;
  if (type.includes("windows") || type.includes("pc")) return Monitor;
  if (type.includes("linux") || type.includes("server")) return Terminal;
  if (type.includes("camera") || type.includes("rtsp")) return Camera;
  if (type.includes("hue") || type.includes("smart home") || svc === "hap") return Lightbulb;
  if (type.includes("cast") || type.includes("chromecast")) return Cast;
  if (type.includes("sonos") || type.includes("audio") || svc === "spotify") return Speaker;
  if (
    type.includes("phone") ||
    type.includes("mobile") ||
    type.includes("android") ||
    type.includes("ios")
  )
    return Smartphone;
  if (type.includes("laptop") || svc === "smb") return Laptop;

  switch (device.deviceType) {
    case "phone":
      return Smartphone;
    case "laptop":
      return Laptop;
    case "printer":
      return Printer;
    case "tv":
      return Tv;
    case "audio":
      return Speaker;
    case "router":
      return Router;
    default:
      return Monitor;
  }
}

function resolveTypeLabel(device: DeviceRow, fingerprintLikelyType: string | null): string {
  const fp = fingerprintLikelyType?.trim() || device.likelyType?.trim();
  if (fp) return fp;
  switch (device.deviceType) {
    case "phone":
      return "Smartphone";
    case "laptop":
      return "Laptop";
    case "printer":
      return "Printer";
    case "tv":
      return "Smart TV";
    case "audio":
      return "Audio Device";
    case "router":
      return "Router";
    case "iot":
      return "IoT Device";
    default:
      return "Network Device";
  }
}

export function DeviceDetailsPanel({
  device,
  customName,
  interrogationName,
  fingerprintLikelyType,
  onSaveCustomName,
  onSaveCustomIcon,
  strings: s,
  showUserMessage,
  onClose,
}: {
  device: DiscoveredDevice;
  customName?: string | null;
  interrogationName?: string | null;
  fingerprintLikelyType: string | null;
  onSaveCustomName: (name: string) => void;
  onSaveCustomIcon: (url: string) => void;
  strings: DeviceDetailsPanelStrings;
  showUserMessage?: (message: string) => void;
  onClose?: () => void;
}) {
  const { dict } = useLanguage();
  const [activeTab, setActiveTab] = useState<"home" | "dns" | "timeline">("home");
  const [isEditing, setIsEditing] = useState(false);
  const [editInput, setEditInput] = useState("");
  const [wolSending, setWolSending] = useState(false);
  const [isDeepScanning, setIsDeepScanning] = useState(false);
  const [deepTotalPorts, setDeepTotalPorts] = useState(0);
  const [deepPortsChecked, setDeepPortsChecked] = useState(0);
  const [deepOpenPorts, setDeepOpenPorts] = useState<number[]>([]);
  const [uploadingIcon, setUploadingIcon] = useState(false);
  const iconInputRef = useRef<HTMLInputElement>(null);
  const deepScanUnlistenRef = useRef<(() => void) | null>(null);

  type HistoryItem = {
    scanId: string;
    scannedAt: number;
    ip: string;
    isOnline: boolean;
    latencyMs: number | null;
  };
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [dnsStats, setDnsStats] = useState<{ total_queries: number; blocked_queries: number } | null>(null);

  useEffect(() => {
    if (device.mac) {
      invoke<HistoryItem[]>("get_device_history", { mac: device.mac, limit: 10 })
        .then(setHistory)
        .catch(console.error);
    }
  }, [device.mac]);

  useEffect(() => {
    if (device.ip) {
      invoke<{ total_queries: number; blocked_queries: number }>("get_device_dns_stats", { ip: device.ip })
        .then(setDnsStats)
        .catch(() => setDnsStats(null));
    }
  }, [device.ip]);

  useEffect(() => {
    return () => {
      deepScanUnlistenRef.current?.();
      deepScanUnlistenRef.current = null;
    };
  }, []);

  const handleIconUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    setUploadingIcon(true);
    const formData = new FormData();
    formData.append("file", file);

    try {
      const res = await transport.fetch("/api/assets/upload", {
        method: "POST",
        body: formData,
      });
      if (res.ok) {
        const { url } = await res.json();
        onSaveCustomIcon(url);
      }
    } catch (e) {
      console.error("Failed to upload icon", e);
    } finally {
      setUploadingIcon(false);
    }
  };

  const displayName =
    customName?.trim() ||
    device.hostname?.trim() ||
    interrogationName?.trim() ||
    device.mdnsHostname?.trim() ||
    device.name?.trim() ||
    device.ip;

  const fingerprint = fingerprintLikelyType?.trim() || device.likelyType?.trim() || null;

  const handleEditSave = () => {
    onSaveCustomName(editInput);
    setIsEditing(false);
  };

  const wolMacCanonical = formatMacForWolIfValid(device.mac?.trim() ?? "");
  const canWakeOnLan =
    Boolean(wolMacCanonical) && typeof window !== "undefined";
  const canDeepScan =
    isValidDeepScanIp(device.ip) && typeof window !== "undefined";
  const deepProgressPct =
    deepTotalPorts > 0
      ? Math.min(100, Math.round((deepPortsChecked / deepTotalPorts) * 100))
      : 0;

  const notify = (message: string) => {
    if (showUserMessage) showUserMessage(message);
    else window.alert(message);
  };

  const handleWakeOnLan = async () => {
    if (!canWakeOnLan || wolSending) return;
    setWolSending(true);
    try {
      const msg = await invoke<string>("wake_on_lan", {
        macAddress: device.mac.trim(),
      });
      notify(msg);
    } catch (err) {
      notify(
        typeof err === "string" ? err : err instanceof Error ? err.message : String(err),
      );
    } finally {
      setWolSending(false);
    }
  };

  const handleDeepScan = async () => {
    if (!canDeepScan || isDeepScanning) return;
    const targetIp = (device?.ip ?? "").trim();
    console.log("[DeepScan] Re-run Port Guardian clicked, device.ip:", device?.ip, "targetIp:", targetIp);
    if (!targetIp) return;
    setIsDeepScanning(true);
    setDeepTotalPorts(0);
    setDeepPortsChecked(0);
    setDeepOpenPorts([]);
    deepScanUnlistenRef.current?.();
    deepScanUnlistenRef.current = null;

    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<DeepPortScanProgressPayload>(
        "deep_scan_progress",
        (event) => {
          if (event.payload.ip !== targetIp) return;
          setDeepTotalPorts(event.payload.totalPorts);
          setDeepPortsChecked(event.payload.portsChecked);
          setDeepOpenPorts([...(event.payload.openPorts ?? [])]);
        },
      );
      deepScanUnlistenRef.current = unlisten;
      try {
        await invoke("run_deep_scan", { ip: targetIp });
      } catch (invokeErr) {
        notify(
          typeof invokeErr === "string"
            ? invokeErr
            : invokeErr instanceof Error
              ? invokeErr.message
              : String(invokeErr),
        );
      }
    } catch (err) {
      notify(
        typeof err === "string" ? err : err instanceof Error ? err.message : String(err),
      );
    } finally {
      unlisten?.();
      if (deepScanUnlistenRef.current === unlisten) deepScanUnlistenRef.current = null;
      setIsDeepScanning(false);
    }
  };

  const Icon = pickDeviceIcon(device);
  const typeLabel = resolveTypeLabel(device, fingerprintLikelyType);

  const isRouter = typeLabel.toLowerCase().includes("router") || typeLabel.toLowerCase().includes("gateway");

  const identityRows = [
    device.hostname?.trim() ? { label: "Hostname", value: device.hostname! } : null,
    device.mdnsHostname?.trim() ? { label: "mDNS", value: device.mdnsHostname! } : null,
    device.ssdpServer?.trim() ? { label: "SSDP Banner", value: device.ssdpServer! } : null,
    fingerprint ? { label: s.likelyType, value: fingerprint } : null,
    interrogationName?.trim() ? { label: "Active ID", value: interrogationName } : null,
  ].filter((r): r is { label: string; value: string } => r !== null);

  const mdnsServiceLabel = device.mdnsPrimaryService
    ? (MDNS_SERVICE_LABELS[device.mdnsPrimaryService.toLowerCase()] ??
      device.mdnsPrimaryService)
    : null;

  const TabButton = ({ 
    tab, 
    label, 
    icon: TabIcon,
    onClickOverride 
  }: { 
    tab?: "home" | "dns" | "timeline", 
    label: string, 
    icon: LucideIcon,
    onClickOverride?: () => void
  }) => (
    <button
      type="button"
      onClick={onClickOverride || (() => tab && setActiveTab(tab))}
      className={cn(
        "flex flex-col items-center gap-1.5 px-3 py-2 text-[10px] font-bold uppercase tracking-wider transition-colors",
        tab === activeTab ? "text-accent" : "text-secondary hover:text-primary"
      )}
    >
      <TabIcon className="w-5 h-5" />
      <span>{label}</span>
    </button>
  );

  return (
    <div className="flex flex-col h-full">
      {/* ── Sub-Tab Header ── */}
      <div className="sticky top-0 z-20 flex items-center justify-around bg-surface/80 backdrop-blur-md border-b border-separator px-2 py-1">
        <TabButton label={dict.tabs.home} icon={Home} tab="home" />
        <TabButton label={dict.tabs.dnsStats} icon={ShieldAlert} tab="dns" />
        <TabButton label={dict.tabs.timelineLog} icon={HistoryIcon} tab="timeline" />
        <TabButton label={dict.tabs.back} icon={ArrowLeft} onClickOverride={onClose} />
      </div>

      <div className="flex-1 overflow-y-auto">
        {activeTab === "home" && (
          <>
            {/* ── Hero ── */}
            <div className="flex flex-col items-center gap-2 pt-6 pb-4">
              <div className="relative group">
                {device.customIcon ? (
                  <img 
                    src={device.customIcon} 
                    alt={displayName} 
                    className="w-16 h-16 rounded-xl object-contain bg-surface-alt p-2 border border-separator" 
                  />
                ) : (
                  <div className="w-16 h-16 rounded-xl bg-surface-alt flex items-center justify-center border border-separator">
                    <Icon className="w-10 h-10 text-secondary" aria-hidden />
                  </div>
                )}
                <button
                  onClick={() => iconInputRef.current?.click()}
                  className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity rounded-xl"
                  title="Change Icon"
                >
                  {uploadingIcon ? (
                    <Loader2 className="w-5 h-5 text-white animate-spin" />
                  ) : (
                    <Camera className="w-5 h-5 text-white" />
                  )}
                </button>
                <input
                  type="file"
                  ref={iconInputRef}
                  onChange={handleIconUpload}
                  accept="image/png,image/jpeg"
                  className="hidden"
                />
              </div>
              <span className="text-[15px] text-secondary">{typeLabel}</span>
              <p className="text-base font-medium text-primary">{displayName}</p>
              {customName?.trim() ? (
                <span className="text-[11px] font-semibold text-accent uppercase tracking-wide">
                  {s.custom}
                </span>
              ) : null}
            </div>

            {/* ── NETWORK ── */}
            <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-4 mb-2">
              Network
            </p>
            <div className="bg-surface rounded-xl overflow-hidden mx-4">
              <div className="flex justify-between items-center px-4 py-3">
                <span className="text-[15px] text-primary">{s.ipAddress}</span>
                {isRouter ? (
                  <a
                    href={`http://${device.ip}`}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-accent text-white text-[13px] font-bold hover:bg-accent-hover transition-colors shadow-sm active:scale-95"
                  >
                    <Router className="w-3.5 h-3.5" />
                    {device.ip}
                  </a>
                ) : (
                  <a
                    href={`http://${device.ip}`}
                    target="_blank"
                    rel="noreferrer"
                    className="text-[15px] font-medium text-accent font-[tabular-nums] hover:text-accent-hover transition-colors underline decoration-accent/30 underline-offset-4"
                  >
                    {device.ip}
                  </a>
                )}
              </div>
              <div className="h-px bg-separator ml-4" />
              <div className="flex justify-between items-center px-4 py-3">
                <span className="text-[15px] text-primary">{s.macAddress}</span>
                <span className="text-[15px] font-medium text-primary font-mono">
                  {device.mac || "—"}
                </span>
              </div>
              <div className="h-px bg-separator ml-4" />
              <div className="flex justify-between items-center px-4 py-3 gap-3">
                <span className="text-[15px] text-primary shrink-0">Vendor</span>
                <span className="text-[15px] font-medium text-primary text-right truncate">
                  {device.vendorName || device.vendor || "—"}
                </span>
              </div>
            </div>

            {/* ── IDENTITY ── */}
            {identityRows.length > 0 && (
              <>
                <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
                  Identity
                </p>
                <div className="bg-surface rounded-xl overflow-hidden mx-4">
                  {identityRows.map((row, i) => (
                    <div key={row.label}>
                      {i > 0 && <div className="h-px bg-separator ml-4" />}
                      <div className="flex justify-between items-center px-4 py-3 gap-3">
                        <span className="text-[15px] text-primary shrink-0">{row.label}</span>
                        <span className="text-[15px] font-medium text-primary text-right truncate">
                          {row.value}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </>
            )}

            {/* ── SERVICES ── */}
            {mdnsServiceLabel && (
              <>
                <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
                  Services
                </p>
                <div className="bg-surface rounded-xl overflow-hidden mx-4">
                  <div className="flex justify-between items-center px-4 py-3">
                    <span className="text-[15px] text-primary">Bonjour / mDNS</span>
                    <span className="text-[15px] font-medium text-primary">{mdnsServiceLabel}</span>
                  </div>
                </div>
              </>
            )}

            {/* ── ACTIONS ── */}
            <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
              Actions
            </p>
            <div className="px-4 space-y-2.5">
              <button
                type="button"
                disabled={!canWakeOnLan || wolSending}
                title={canWakeOnLan ? s.wakeOnLan : s.wakeOnLanNeedMac}
                onClick={() => void handleWakeOnLan()}
                className={cn(
                  "w-full py-3.5 rounded-xl bg-accent text-white text-[16px] font-semibold text-center transition-colors",
                  "hover:bg-accent-hover active:brightness-90 disabled:opacity-50 disabled:cursor-not-allowed",
                )}
              >
                {wolSending ? (
                  <span className="inline-flex items-center justify-center gap-2">
                    <Loader2 className="w-4 h-4 animate-spin" aria-hidden />
                    {s.wakeOnLan}
                  </span>
                ) : (
                  s.wakeOnLan
                )}
              </button>
              <button
                type="button"
                disabled={!canDeepScan || isDeepScanning}
                title={canDeepScan ? s.runDeepScan : s.deepScanNeedIp}
                onClick={() => void handleDeepScan()}
                className={cn(
                  "w-full py-3.5 rounded-xl bg-surface-alt text-primary text-[16px] font-semibold text-center transition-colors",
                  "hover:bg-surface-hover active:brightness-90 disabled:opacity-50 disabled:cursor-not-allowed",
                )}
              >
                {isDeepScanning ? (
                  <span className="inline-flex items-center justify-center gap-2">
                    <Loader2 className="w-4 h-4 animate-spin" aria-hidden />
                    {s.runDeepScan}
                  </span>
                ) : (
                  s.runDeepScan
                )}
              </button>
            </div>

            {/* ── DEEP SCAN PORTS (live) ── */}
            {(isDeepScanning || deepTotalPorts > 0) && (
              <>
                <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
                  {s.deepScanOpenPortsTitle}
                </p>
                <div className="px-4 space-y-3">
                  {deepTotalPorts > 0 && (
                    <div className="flex items-center justify-between text-[13px] text-secondary">
                      <span>
                        {s.deepScanPortsProgress
                          .replace("{checked}", String(deepPortsChecked))
                          .replace("{total}", String(deepTotalPorts))}
                      </span>
                      <span className="font-[tabular-nums]">{deepProgressPct}%</span>
                    </div>
                  )}
                  <div
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={deepProgressPct}
                    className="h-1 w-full overflow-hidden rounded-full bg-surface-alt"
                  >
                    <div
                      className="h-full rounded-full bg-accent transition-[width] duration-150 ease-out"
                      style={{ width: `${deepProgressPct}%` }}
                    />
                  </div>
                  {deepOpenPorts.length > 0 && (
                    <div className="bg-surface rounded-xl overflow-hidden">
                      {deepOpenPorts.map((p, i) => (
                        <div key={p}>
                          {i > 0 && <div className="h-px bg-separator ml-4" />}
                          <div className="flex justify-between items-center px-4 py-3">
                            <span className="text-[15px] font-mono font-medium text-primary">
                              {p}
                            </span>
                            <span className="text-[15px] text-secondary">
                              {DEEP_PORT_LABELS[p] ?? "Unknown"}
                            </span>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </>
            )}

            {/* ── LABEL (custom name) ── */}
            <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mt-6 mb-2">
              Label
            </p>
            <div className="bg-surface rounded-xl overflow-hidden mx-4">
              {!isEditing ? (
                <button
                  type="button"
                  onClick={() => {
                    setEditInput(customName?.trim() ?? "");
                    setIsEditing(true);
                  }}
                  className="w-full flex justify-between items-center px-4 py-3 active:bg-surface-hover transition-colors"
                >
                  <span className="text-[15px] text-primary">
                    {customName?.trim() ? s.editName : s.setCustomName}
                  </span>
                  <span className="text-[15px] text-secondary truncate max-w-[55%] text-right">
                    {customName?.trim() || s.noCustomName}
                  </span>
                </button>
              ) : (
                <div className="px-4 py-3 space-y-3">
                  <input
                    type="text"
                    value={editInput}
                    onChange={(e) => setEditInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleEditSave();
                      if (e.key === "Escape") setIsEditing(false);
                    }}
                    placeholder={s.customNamePlaceholder}
                    autoFocus
                    className="w-full rounded-lg bg-surface-alt px-3 py-2 text-[15px] text-primary placeholder:text-tertiary outline-none focus:ring-1 focus:ring-accent"
                  />
                  <div className="flex gap-2">
                    <button
                      type="button"
                      onClick={handleEditSave}
                      className="flex-1 py-2 rounded-lg bg-accent text-white text-[13px] font-semibold hover:bg-accent-hover transition-colors"
                    >
                      {s.save}
                    </button>
                    <button
                      type="button"
                      onClick={() => setIsEditing(false)}
                      className="px-4 py-2 rounded-lg bg-surface-alt text-secondary text-[13px] font-medium transition-colors"
                    >
                      {s.cancel}
                    </button>
                    {customName?.trim() ? (
                      <button
                        type="button"
                        onClick={() => {
                          onSaveCustomName("");
                          setIsEditing(false);
                        }}
                        className="px-4 py-2 rounded-lg bg-surface-alt text-error text-[13px] font-medium transition-colors"
                      >
                        {s.clear}
                      </button>
                    ) : null}
                  </div>
                </div>
              )}
            </div>
          </>
        )}

        {activeTab === "dns" && (
          <div className="pt-6">
            <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mb-2">
              Security / DNS Details
            </p>
            {dnsStats ? (
              <div className="bg-surface rounded-xl overflow-hidden mx-4">
                <div className="flex justify-between items-center px-4 py-3">
                  <span className="text-[15px] text-primary">Total Queries</span>
                  <span className="text-[15px] font-medium text-primary font-[tabular-nums]">
                    {dnsStats.total_queries.toLocaleString()}
                  </span>
                </div>
                <div className="h-px bg-separator ml-4" />
                <div className="flex justify-between items-center px-4 py-3">
                  <span className="text-[15px] text-primary">Blocked Queries</span>
                  <div className="flex flex-col items-end">
                    <span className="text-[15px] font-semibold text-error font-[tabular-nums]">
                      {dnsStats.blocked_queries.toLocaleString()}
                    </span>
                    {dnsStats.total_queries > 0 && (
                      <span className="text-[12px] text-tertiary">
                        {((dnsStats.blocked_queries / dnsStats.total_queries) * 100).toFixed(1)}% blocked
                      </span>
                    )}
                  </div>
                </div>
              </div>
            ) : (
              <p className="px-4 py-8 text-center text-sm text-secondary italic">
                No DNS statistics available for this node.
              </p>
            )}
          </div>
        )}

        {activeTab === "timeline" && (
          <div className="pt-6">
            <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-4 mb-2">
              Timeline Log
            </p>
            {history.length > 0 ? (
              <div className="bg-surface rounded-xl overflow-hidden mx-4">
                {history.map((h, i) => (
                  <div key={h.scanId || i}>
                    {i > 0 && <div className="h-px bg-separator ml-4" />}
                    <div className="flex justify-between items-center px-4 py-3">
                      <div className="flex flex-col">
                        <span className="text-[15px] text-primary">
                          {new Date(h.scannedAt).toLocaleString()}
                        </span>
                        <span className="text-[12px] text-tertiary font-mono">{h.ip}</span>
                      </div>
                      <div className="flex flex-col items-end">
                        <span
                          className={cn(
                            "text-[14px] font-semibold",
                            h.isOnline ? "text-online" : "text-error",
                          )}
                        >
                          {h.isOnline ? "Online" : "Offline"}
                        </span>
                        {h.latencyMs != null && (
                          <span className="text-[12px] text-tertiary">
                            {h.latencyMs.toFixed(1)} ms
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <p className="px-4 py-8 text-center text-sm text-secondary italic">
                No historical timeline data recorded.
              </p>
            )}
          </div>
        )}
      </div>

      <div className="h-6 shrink-0" />
    </div>
  );
}

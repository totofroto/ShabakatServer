import type { KeyboardEvent, MouseEvent, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import {
  Camera,
  Cast,
  Cpu,
  Database,
  Edit2,
  Gamepad2,
  GitBranch,
  HardDrive,
  Headphones,
  HelpCircle,
  Home,
  Laptop,
  Lightbulb,
  Loader2,
  Mic,
  Monitor,
  Play,
  Printer,
  RadioTower,
  Refrigerator,
  Router,
  Server,
  Smartphone,
  Speaker,
  Terminal,
  Tv,
  Volume2,
  Wifi,
  Zap,
} from "lucide-react";
import type { DeviceRow } from "@/hooks/useNetworkScan";
import { cn } from "@/lib/utils";

// ── Pure data logic (keep unchanged per CLAUDE.md) ───────────────────────────

/** List rows: displayName first, then interrogationName, hostname, mdnsHostname, vendor. */
export function getDeviceListPrimaryLine(
  device: DeviceRow,
  customName: string | undefined,
  _unknownLabel: string,
): { primary: string; primaryClassName?: string; prominent: boolean } {
  // Priority 1: any user/server-set display name (localStorage override > server customName > server displayName)
  const dn = customName?.trim() || device.customName?.trim() || device.displayName?.trim();
  if (dn) return { primary: dn, prominent: true };
  const ig = device.interrogationName?.trim();
  if (ig) return { primary: ig, prominent: true };
  const hn = device.hostname?.trim();
  if (hn) return { primary: hn, prominent: true };
  const mdns = device.mdnsHostname?.trim();
  if (mdns) return { primary: mdns, prominent: true };
  const nm = device.name?.trim() ?? "";
  const nmLower = nm.toLowerCase();
  const nameIsGeneric =
    !nm || nm === device.ip || nmLower === "unknown" || nmLower.startsWith("host ");
  if (!nameIsGeneric) return { primary: nm, prominent: true };
  if (
    !device.mdnsHostname?.trim() &&
    nameIsGeneric &&
    Boolean(device.vendorName?.trim()) &&
    device.vendorName !== "Unknown"
  ) {
    return { primary: device.vendorName!, prominent: true };
  }
  return { primary: device.ip, prominent: false };
}

/** Smaller subline: IP and fingerprint / class (Port Guardian + scan context). */
export function getDeviceListMetaLine(device: DeviceRow): string {
  let typePart =
    device.likelyType?.trim() ||
    device.deviceType ||
    (device.vendorName && device.vendorName !== "Unknown" ? device.vendorName : null) ||
    (device.ssdpServer ? ssdpBannerShort(device.ssdpServer) : null) ||
    "—";
  // Fingerprinting sometimes misclassifies non-gateway hosts as "Router / Gateway".
  // Only the last octet .1 or .2 is a plausible default gateway; everything else
  // that got that label is more accurately shown as "Network Device".
  if (typePart.includes("Router") || typePart.includes("Gateway")) {
    const lastOctet = parseInt(device.ip.split(".").pop() ?? "999", 10);
    if (lastOctet > 2) typePart = "Network Device";
  }
  return `${device.ip} · ${typePart}`;
}

/** Extract the most useful token from a raw SSDP SERVER: banner for display. */
function ssdpBannerShort(banner: string): string {
  const t = banner.trim();
  const upnpIdx = t.toUpperCase().indexOf("UPNP/");
  if (upnpIdx !== -1) {
    const tail = t.slice(upnpIdx + 5).trim();
    const tok = tail.split(/\s+/)[1] ?? tail.split(/\s+/)[0] ?? "";
    if (tok && tok.includes("/") && tok.length <= 32) return `UPnP · ${tok}`;
  }
  const tok = t
    .split(/\s+/)
    .find((p) => p.includes("/") && !/^(Linux|Windows|Unix|Darwin|Android)\//i.test(p));
  return tok ? `UPnP · ${tok}` : "UPnP Device";
}

function formatLastSeen(ts: number): string {
  const diff = Date.now() - ts;
  const sec = Math.floor(diff / 1000);
  if (sec < 90) return "Last seen: just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `Last seen: ${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `Last seen: ${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day === 1) return "Last seen: yesterday";
  return `Last seen: ${day}d ago`;
}

// ── Icon mapping (logic unchanged from devices-page.tsx) ─────────────────────

/** Maps a likelyType string → Lucide icon. Returns null when no rule matches. */
export function iconFromLikelyType(likelyType: string | null | undefined): LucideIcon | null {
  const t = likelyType?.toLowerCase() ?? "";
  if (!t) return null;
  // Gaming
  if (t.includes("playstation")) return Gamepad2;
  // AV / Audio
  if (t.includes("av receiver") || t.includes("receiver")) return Volume2;
  if (t.includes("amazon echo") || t.includes("echo dot")) return Mic;
  if (t.includes("audio dac")) return Headphones;
  if (t.includes("sonos") || t.includes("audio") || t.includes("speaker")) return Speaker;
  // TV / Streaming / Remotes
  if (t.includes("chromecast") || t.includes("google tv")) return Cast;
  if (t.includes("apple tv") || t.includes("homepod") || t.includes("airplay")) return Tv;
  if (t.includes("tv remote") || t.includes("smart remote")) return Tv;
  if (t.includes("smart tv") || t.includes("samsung smart") || t.includes("samsung") || t.includes("lg tv") || t.includes("lg smart")) return Tv;
  // Camera
  if (t.includes("camera") || t.includes("rtsp")) return Camera;
  // Printing
  if (t.includes("printer") || t.includes("jetdirect") || t.includes("ipp") || t.includes("lpd")) return Printer;
  // Storage
  if (t.includes("synology") || t.includes("asustor") || t.includes("nas")) return HardDrive;
  // Media server
  if (t.includes("plex")) return Play;
  // Network infrastructure
  if (t.includes("ubiquiti") || t.includes("unifi")) return Wifi;
  if (t.includes("router") || t.includes("gateway")) return Router;
  if (t.includes("dns") || t.includes("pi-hole")) return Router;
  if (t.includes("switch")) return GitBranch;
  // Computers
  if (t.includes("linux server") || t.includes("raspberry pi")) return Terminal;
  if (t.includes("windows pc") || t.includes("windows")) return Monitor;
  if (t.includes("macbook") || t.includes("mac / apple") || t.includes("apple device")) return Laptop;
  if (t.includes("iphone") || t.includes("ipad") || t.includes("android")) return Smartphone;
  // Smart home
  if (t.includes("home assistant")) return Home;
  if (t.includes("smart appliance")) return Refrigerator;
  if (t.includes("smart light") || t.includes("yeelink")) return Lightbulb;
  if (t.includes("philips hue")) return Lightbulb;
  // IoT / DB / Web (lower priority)
  if (t.includes("mqtt") || t.includes("iot hub")) return Cpu;
  if (t.includes("mysql") || t.includes("postgresql") || t.includes("mongodb")) return Database;
  if (t.includes("http") || t.includes("https") || t.includes("web interface") || t.includes("admin panel")) return Server;
  return null;
}

export function pickDeviceIcon(device: DeviceRow): LucideIcon {
  const fromType = iconFromLikelyType(device.likelyType);
  if (fromType) return fromType;
  switch (device.deviceType) {
    case "phone":   return Smartphone;
    case "laptop":  return Laptop;
    case "printer": return Printer;
    case "tv":      return Tv;
    case "audio":   return Speaker;
    case "iot":     return Cpu;
    default:        return HelpCircle;
  }
}

// ── Visual components (design system §5.2) ────────────────────────────────────

/**
 * Status dot: 8×8 circle indicating device lifecycle state.
 * Green for last_seen < 5 minutes, Grey for last_seen > 5 minutes.
 */
export function DeviceStatusDot({ device }: { device: DeviceRow }) {
  const isRecent = device.lastSeen && (Date.now() - device.lastSeen < 5 * 60 * 1000);
  if (isRecent) {
    return <span className="w-2 h-2 rounded-full bg-online shrink-0" aria-hidden />;
  }
  return <span className="w-2 h-2 rounded-full bg-tertiary shrink-0" aria-hidden />;
}

/** Raw Lucide device icon or custom image — no background circle, no color (text-secondary). */
export function DeviceTypeIcon({ device }: { device: DeviceRow }) {
  if (device.customIcon) {
    return (
      <img 
        src={device.customIcon} 
        alt="" 
        className="w-5 h-5 shrink-0 rounded object-contain" 
        aria-hidden 
      />
    );
  }
  const Icon = pickDeviceIcon(device);
  return <Icon className="w-5 h-5 shrink-0 text-secondary" aria-hidden />;
}

/** Middle zone: primary device name + meta subline. */
export function DeviceListNameHeading({
  primary,
  secondary,
  prominent: _prominent,
  primaryClassName,
  children,
}: {
  primary: string;
  secondary: string | null;
  prominent: boolean;
  primaryClassName?: string;
  children?: ReactNode;
}) {
  return (
    <div className="min-w-0 flex-1">
      <p
        className={cn(
          "truncate text-base font-medium text-primary leading-tight",
          primaryClassName,
        )}
        title={primary}
      >
        {primary}
      </p>
      {secondary ? (
        <p className="truncate text-[13px] text-secondary mt-0.5" title={secondary}>
          {secondary}
        </p>
      ) : null}
      {children}
    </div>
  );
}

/** "Last seen Xm ago" shown below the meta line for offline devices. */
export function DeviceLastSeenLabel({ lastSeen }: { lastSeen: number | null | undefined }) {
  if (!lastSeen) return null;
  return (
    <p className="truncate text-[13px] text-secondary mt-0.5">{formatLastSeen(lastSeen)}</p>
  );
}

/** "Offline" badge for devices seen > 10m ago. */
export function DeviceOfflineBadge({ lastSeen }: { lastSeen: number | null | undefined }) {
  if (!lastSeen) return null;
  const isStale = Date.now() - lastSeen > 10 * 60 * 1000;
  if (!isStale) return null;
  
  return (
    <span className="inline-flex items-center rounded-full bg-surface-alt border border-separator px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-tertiary ml-2">
      Offline
    </span>
  );
}

/** Wake-on-LAN button — surface tokens, accent Zap icon, no glow. */
export function DeviceListWakeButton({
  isSending,
  onClick,
  label,
  sendingLabel,
}: {
  isSending: boolean;
  onClick: (e: MouseEvent<HTMLButtonElement>) => void;
  label: string;
  sendingLabel: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={isSending}
      className={cn(
        "mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5",
        "bg-surface-alt border border-separator",
        "text-[12px] font-medium text-primary",
        "hover:bg-surface-hover transition-colors duration-100",
        "disabled:opacity-50 disabled:cursor-wait",
        "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent/50",
      )}
    >
      {isSending ? (
        <Loader2 className="w-3 h-3 shrink-0 animate-spin" aria-hidden />
      ) : (
        <Zap className="w-3 h-3 shrink-0 text-accent" aria-hidden />
      )}
      {isSending ? sendingLabel : label}
    </button>
  );
}

/** Link to device web portal — Section 5.12 style. */
export function DeviceLaunchPortalLink({
  ip,
  label,
}: {
  ip: string;
  label: string;
}) {
  return (
    <a
      href={`http://${ip}`}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(e) => e.stopPropagation()}
      className={cn(
        "mt-1 inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5",
        "bg-surface-alt border border-separator",
        "text-[12px] font-medium text-primary",
        "hover:bg-surface-hover transition-colors duration-100",
        "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent/50",
      )}
    >
      <RadioTower className="w-3 h-3 shrink-0 text-accent" aria-hidden />
      {label}
    </a>
  );
}

/**
 * Row shell: flat tap target — no card border/shadow.
 * The parent container supplies the card (bg-surface rounded-xl).
 * scoreGlow prop is kept for API compatibility but not rendered.
 */
export function DeviceRowShell({
  device,
  isSelected,
  scoreGlow: _scoreGlow,
  onClick,
  onKeyDown,
  children,
}: {
  device: DeviceRow;
  isSelected: boolean;
  scoreGlow: boolean;
  onClick: () => void;
  onKeyDown: (e: KeyboardEvent<HTMLDivElement>) => void;
  children: ReactNode;
}) {
  if (!device || typeof device.ip !== "string" || device.ip.trim() === "") {
    return null;
  }
  const isRecent = device.lastSeen && (Date.now() - device.lastSeen < 5 * 60 * 1000);

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={onKeyDown}
      data-online={device.isOnline ? "true" : "false"}
      className={cn(
        "flex items-center gap-3 px-4 py-3.5 cursor-pointer transition-colors duration-100",
        "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent/50",
        isSelected ? "bg-surface-hover" : "hover:bg-surface-hover",
        !isRecent && "opacity-60",
      )}
    >
      {children}
    </div>
  );
}

/** @deprecated Status is now shown via DeviceStatusDot. */
export function DeviceNewBadge({ show: _show }: { show: boolean; label?: string }) {
  return null;
}

/** @deprecated Status is now shown via DeviceStatusDot. */
export function DeviceMissingBadge({ show: _show }: { show: boolean; label?: string }) {
  return null;
}

/** Edit button for device alias — Section 5.12 style. */
export function DeviceListEditButton({
  onClick,
  title,
}: {
  onClick: (e: MouseEvent<HTMLButtonElement>) => void;
  title: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      className={cn(
        "inline-flex items-center justify-center rounded-lg p-2",
        "text-tertiary hover:bg-surface-alt hover:text-primary transition-colors duration-100",
        "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent/50",
      )}
    >
      <Edit2 className="w-4 h-4 shrink-0" aria-hidden />
    </button>
  );
}

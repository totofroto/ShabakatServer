import { invoke, isTauri, listen } from "@/lib/transport";
import {
  checkPermissions,
  requestPermissions,
} from "@tauri-apps/plugin-geolocation";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { Store } from "@tauri-apps/plugin-store";
import { useCallback, useEffect, useRef, useState } from "react";
import { useDeviceStore } from "@/stores/deviceStore";

/** Kept in sync with `PLUGIN_STORE_DEVICES_FILE` in `src-tauri/src/monitor.rs` (JSON on disk, not .bin). */
export const DEVICES_STORE_FILE = "devices.json";

// ── Session-level latency sample cache ────────────────────────────────────────
// Persists ping samples across device panel open/close within a session.
// Keyed by deviceHistoryUniqueId (MAC when real, IP otherwise) so that
// IP→MAC identity upgrades can migrate samples to the stable key.

const latencySampleCache = new Map<string, number[]>();
const LATENCY_SAMPLE_MAX = 32;
const LATENCY_CACHE_MAX_ENTRIES = 50;

export function getLatencySamples(mac: string, ip: string): number[] {
  return latencySampleCache.get(deviceHistoryUniqueId(mac, ip)) ?? [];
}

export function addLatencySample(mac: string, ip: string, ms: number): number[] {
  const key = deviceHistoryUniqueId(mac, ip);
  const arr = latencySampleCache.get(key) ?? [];
  arr.push(ms);
  if (arr.length > LATENCY_SAMPLE_MAX) arr.shift();
  latencySampleCache.set(key, arr);
  return arr;
}

export function migrateLatencyKey(oldKey: string, newKey: string): void {
  if (oldKey === newKey || !latencySampleCache.has(oldKey)) return;
  const samples = latencySampleCache.get(oldKey)!;
  const existing = latencySampleCache.get(newKey) ?? [];
  latencySampleCache.set(newKey, [...existing, ...samples].slice(-LATENCY_SAMPLE_MAX));
  latencySampleCache.delete(oldKey);
}

export function evictLatencyCache(): void {
  if (latencySampleCache.size <= LATENCY_CACHE_MAX_ENTRIES) return;
  const toDelete = latencySampleCache.size - LATENCY_CACHE_MAX_ENTRIES;
  let deleted = 0;
  for (const key of latencySampleCache.keys()) {
    latencySampleCache.delete(key);
    if (++deleted >= toDelete) break;
  }
}

/** Derived lifecycle state for display and map coloring (not persisted). */
export function deviceLifecycleState(
  device: Pick<DeviceRow, "isOnline" | "lastSeen">,
): "online" | "stale" | "offline" {
  if (device.isOnline) return "online";
  if (device.lastSeen && Date.now() - device.lastSeen < 5 * 60 * 1000) return "stale";
  return "offline";
}
export const DEVICES_STORE_KEY = "devices";

const DEVICE_STORE_OPTIONS = { defaults: {}, autoSave: false as const };

/** Caches the native store handle; cleared on Vite HMR so it cannot desync from the Rust side. */
let storeInstance: Store | null = null;
let storeLoadPromise: Promise<Store> | null = null;

/**
 * Async store init. `@tauri-apps/plugin-store` requires `Store.load(...)`; there is no `new Store(path)`.
 * Concurrent callers share one in-flight `Store.load`; successful load sets `storeInstance`.
 */
async function getStore(): Promise<Store> {
  if (!isTauri() || typeof window === "undefined") {
    throw new Error("getStore: not available without Tauri");
  }
  if (storeInstance) {
    return storeInstance;
  }
  if (!storeLoadPromise) {
    storeLoadPromise = Store.load(DEVICES_STORE_FILE, DEVICE_STORE_OPTIONS)
      .then((store) => {
        storeInstance = store;
        return store;
      })
      .catch((err) => {
        storeInstance = null;
        storeLoadPromise = null;
        throw err;
      });
  }
  return storeLoadPromise;
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    storeInstance = null;
    storeLoadPromise = null;
  });
}

export type DeviceStatus = "Online" | "Offline" | "Warning";
export type ScanMode = "silent" | "aggressive";

export type DeviceRow = {
  status: DeviceStatus;
  name: string;
  ip: string;
  mac: string;
  vendor: string;
  /** IEEE OUI company name from bundled mac-vendors.json (mirrors Rust `vendor_name`). */
  vendorName: string;
  deviceType: string;
  isRandomized: boolean;
  /** mDNS hostname when Zeroconf resolved (e.g. Kitchen-Speaker). */
  mdnsHostname?: string | null;
  /** Zeroconf category for UI: googlecast, hap, printer, spotify, smb. */
  mdnsPrimaryService?: string | null;
  /** True when quick port scan found SSH (22) or HTTP (80) open. */
  shieldHighlight?: boolean;
  /** Port-fingerprint guess (e.g. "Windows PC"). Populated after a port scan. */
  likelyType?: string | null;
  /** HTTP banner / unicast mDNS active-interrogation label (e.g. "FRITZ!Box 7590"). */
  interrogationName?: string | null;
  /** Reverse-DNS (PTR / OS resolver) hostname; not Zeroconf (see `mdnsHostname`). */
  hostname?: string | null;
  /** Raw SSDP SERVER: banner (e.g. "Linux/3.14 UPnP/1.0 Sonos/58.0"). Present whenever the
   *  device responded to our UPnP M-SEARCH — valuable on Android where MAC is hidden. */
  ssdpServer?: string | null;
  /** User-assigned label from the Identity Vault (localStorage). */
  customName?: string | null;
  /** Server-stored display name (set via PATCH /api/devices/:mac or by the scan engine). */
  displayName?: string | null;
  /** Custom mapped icon URL for this device. */
  customIcon?: string | null;
  /** Host replied on the current / last completed scan pass. */
  isOnline: boolean;
  /** First time this identity (MAC / IP key) appeared in our persisted history. */
  isNew: boolean;
  /** Unix epoch ms of the most recent scan in which this device was seen online. Null for pre-history rows. */
  lastSeen?: number | null;
  /** Unix epoch ms when this device was first added to persisted history. Null for legacy rows loaded before this field existed. */
  firstSeen?: number | null;
};

type DiscoveredDevicePayload = {
  status: string;
  name: string;
  ip: string;
  mac: string;
  vendor: string;
  vendorName?: string | null;
  deviceType: string;
  isRandomized: boolean;
  mdnsHostname?: string | null;
  mdnsPrimaryService?: string | null;
  /** Port-fingerprint label from TCP-fallback probing during liveness scan. Matches Rust `likely_type` (serialized as camelCase). */
  likelyType?: string | null;
  /** Reverse-DNS hostname from the Rust scan (camelCase in IPC). */
  hostname?: string | null;
  /** Raw SSDP SERVER: banner from UPnP M-SEARCH response. */
  ssdpServer?: string | null;
};

type ScanStartedPayload = {
  /** Monotonic ID emitted by Rust at the very top of scan_network, before discovery. */
  scanId: string;
};

type ScanNetworkPayload = {
  devices: DiscoveredDevicePayload[];
  averageLatencyMs: number | null;
  scannedHosts: number;
  totalHosts: number;
  /** Same ID that was broadcast in scan_started — present on every discovery event. */
  scanId?: string | null;
  /** 1-based batch sequence number within the scan; 0 for non-batch events. */
  batchSeq?: number;
};

type AndroidPermissionResult = {
  status: "granted" | "denied";
  fineLocation: boolean;
  nearbyWifiDevices: boolean;
  coarseLocation: boolean;
};


function normalizeStatus(raw: string): DeviceStatus {
  if (raw === "Online" || raw === "Offline" || raw === "Warning") {
    return raw;
  }
  return "Online";
}

const CUSTOM_NAMES_LS_KEY = "shabakat_custom_names";

function readCustomNamesFromStorage(): Record<string, string> {
  try {
    return JSON.parse(
      localStorage.getItem(CUSTOM_NAMES_LS_KEY) ?? "{}",
    ) as Record<string, string>;
  } catch {
    return {};
  }
}

/**
 * Raw device identity: usable MAC from the OS when available; otherwise `ip`
 * (Android 11+ often hides ARP / MACs).
 * Matches: `(mac && mac !== "Unknown" && mac !== "MAC Restricted") ? mac : ip` (case-fold "mac restricted").
 */
export function deviceHistoryUniqueId(
  mac: string | undefined | null,
  ip: string | undefined | null,
): string {
  const m = (mac ?? "").trim();
  if (
    m &&
    m !== "Unknown" &&
    m !== "MAC Restricted" &&
    m.toLowerCase() !== "mac restricted"
  ) {
    return m;
  }
  return (ip ?? "").trim();
}

/**
 * 12 hex digits, not all-zero, excluding OS placeholders. Matches rules used for
 * Wake-on-LAN in the app (see device details / tools).
 */
export function isValidMacForWakeOnLan(mac: string | undefined | null): boolean {
  if (!mac?.trim()) {
    return false;
  }
  const low = mac.trim().toLowerCase();
  if (low === "unknown" || low === "mac restricted") {
    return false;
  }
  const hex = mac.replace(/[^0-9a-fA-F]/g, "");
  if (hex.length !== 12) {
    return false;
  }
  if (/^0{12}$/i.test(hex)) {
    return false;
  }
  return true;
}

/** `historyMap` key — prefixes `deviceHistoryUniqueId` for stable Map lookup. */
export function deviceHistoryKey(mac: string, ip: string): string {
  const m = (mac ?? "").trim();
  const ipT = (ip ?? "").trim();
  const uid = deviceHistoryUniqueId(m, ipT);
  if (uid === ipT) {
    return `ip:${ipT}`;
  }
  const compact = uid.replace(/[^0-9a-fA-F]/gi, "").toUpperCase();
  if (compact.length === 12) {
    return `mac:${compact}`;
  }
  return `mac:${uid.toLowerCase()}`;
}

function hasStableMac(mac: string): boolean {
  return deviceHistoryUniqueId(mac, "0.0.0.0") === (mac ?? "").trim();
}

/** Drop prior `ip:` history row when the same host now reports a real MAC. */
function consolidateIpKeyForMacDevice(
  historyMap: Map<string, DeviceRow>,
  inc: DeviceRow,
): void {
  if (!hasStableMac(inc.mac)) {
    return;
  }
  const drop: string[] = [];
  for (const [k, d] of historyMap) {
    if (
      k.startsWith("ip:") &&
      d.ip.trim() === inc.ip.trim() &&
      !hasStableMac(d.mac)
    ) {
      drop.push(k);
    }
  }
  for (const k of drop) {
    historyMap.delete(k);
  }
}

function normalizeStoredDeviceRow(raw: unknown): DeviceRow | null {
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const d = raw as Partial<DeviceRow>;
  if (typeof d.ip !== "string" || !d.ip) {
    return null;
  }
  const mac = typeof d.mac === "string" ? d.mac : "Unknown";
  const isOnline = Boolean(d.isOnline);
  const status = normalizeStatus(
    typeof d.status === "string" ? d.status : isOnline ? "Online" : "Offline",
  );
  return {
    status: isOnline ? status : "Offline",
    name: typeof d.name === "string" ? d.name : "",
    ip: d.ip,
    mac,
    vendor: typeof d.vendor === "string" ? d.vendor : "",
    vendorName:
      typeof d.vendorName === "string" ? d.vendorName : "Unknown",
    deviceType: typeof d.deviceType === "string" ? d.deviceType : "unknown",
    isRandomized: Boolean(d.isRandomized),
    mdnsHostname: d.mdnsHostname ?? null,
    mdnsPrimaryService: d.mdnsPrimaryService ?? null,
    shieldHighlight: d.shieldHighlight,
    likelyType: d.likelyType ?? null,
    interrogationName: d.interrogationName ?? null,
    hostname: typeof d.hostname === "string" ? d.hostname : null,
    customName: d.customName ?? null,
    isOnline,
    // Disk snapshot = already part of history; badge is for fresh discoveries this session.
    isNew: false,
    lastSeen: typeof d.lastSeen === "number" ? d.lastSeen : null,
    firstSeen: typeof d.firstSeen === "number" ? d.firstSeen : null,
  };
}

function mapDiscoveredToRow(
  p: DiscoveredDevicePayload,
  customNames: Record<string, string>,
): DeviceRow {
  const vendorName = p.vendorName?.trim() || p.vendor?.trim() || "Unknown";
  return {
    status: normalizeStatus(p.status),
    name: p.name,
    ip: p.ip,
    mac: p.mac,
    vendor: p.vendor,
    vendorName,
    deviceType: p.deviceType,
    isRandomized: p.isRandomized,
    mdnsHostname: p.mdnsHostname ?? null,
    mdnsPrimaryService: p.mdnsPrimaryService ?? null,
    likelyType: p.likelyType?.trim() || null,
    hostname: p.hostname?.trim() || null,
    ssdpServer: p.ssdpServer?.trim() || null,
    customName: customNames[p.ip]?.trim() || null,
    isOnline: true,
    isNew: false,
    lastSeen: Date.now(),
  };
}

/**
 * Secondary ordering: known infrastructure / media before generic "Unknown" types.
 * Lower = earlier in the list (within the same online/offline tier).
 */
function deviceCategorySortRank(d: DeviceRow): number {
  const blob = [
    d.likelyType,
    d.deviceType,
    d.name,
    d.vendorName,
    d.hostname,
    d.mdnsPrimaryService,
  ]
    .map((s) => (s ?? "").trim().toLowerCase())
    .filter(Boolean)
    .join(" ");
  if (
    /(router|gateway|modem|fritz!|eero|orbi|unifi|mikrotik|netgate|gpon|fortinet|ont\b|broadband|xfinity|netgear|linksys|asus|nighthawk|access[\s-]?point|firewall|mesh|fiber)/.test(
      blob,
    )
  ) {
    return 0;
  }
  if (
    /(chromecast|roku|fire\s*tv|android\s*tv|smart[\s-]?tv|\btv\b|appletv|samsung(?!.*phone)|entertainment|display)/.test(
      blob,
    ) ||
    d.mdnsPrimaryService === "googlecast"
  ) {
    return 1;
  }
  const lt = d.likelyType?.trim().toLowerCase() ?? "";
  if (lt && !lt.includes("unknown")) {
    return 2;
  }
  const dt = d.deviceType?.trim().toLowerCase() ?? "";
  if (dt && dt !== "unknown") {
    return 3;
  }
  return 4;
}

function sortDevicesForDisplay(rows: DeviceRow[]): DeviceRow[] {
  return [...rows].sort((a, b) => {
    if (a.isOnline !== b.isOnline) {
      return a.isOnline ? -1 : 1;
    }
    const ra = deviceCategorySortRank(a);
    const rb = deviceCategorySortRank(b);
    if (ra !== rb) {
      return ra - rb;
    }
    return a.ip.localeCompare(b.ip, undefined, { numeric: true });
  });
}

function markHistoryOffline(historyMap: Map<string, DeviceRow>): void {
  for (const [k, d] of historyMap) {
    historyMap.set(k, {
      ...d,
      isOnline: false,
      isNew: false,
      status: "Offline",
    });
  }
}

type MergeScanProgressResult = {
  devices: DeviceRow[];
  /** Devices that are genuinely new in `historyMap` and should show intruder alerts. */
  newIntruderNotifications: DeviceRow[];
};

/**
 * Merges one batch of discovered devices into the persistent `historyMap` (keyed
 * by `deviceHistoryKey` / `uniqueHistoryIdentity`). Never replaces the map wholesale.
 */
function mergeScanProgress(
  historyMap: Map<string, DeviceRow>,
  incoming: DeviceRow[],
): MergeScanProgressResult {
  const valid = incoming.filter(
    (d) => typeof d.ip === "string" && d.ip.length > 0,
  );
  const names = readCustomNamesFromStorage();
  /** No alerts on the first ever baseline (empty map) — only after we have known hosts. */
  const hadPriorHistory = historyMap.size > 0;
  const newIntruderNotifications: DeviceRow[] = [];

  for (let inc of valid) {
    // When the scanner can't resolve ARP (mac = "Unknown"), recover the real MAC
    // from any existing historyMap entry for the same IP so the DeviceRow keeps
    // the known MAC rather than creating a duplicate "Unknown" entry.
    if (!hasStableMac(inc.mac) && inc.ip.trim()) {
      for (const [, d] of historyMap) {
        if (d.ip.trim() === inc.ip.trim() && hasStableMac(d.mac)) {
          inc = { ...inc, mac: d.mac };
          break;
        }
      }
    }

    const ipOnlyKey = `ip:${inc.ip.trim()}`;
    const priorIpOnly =
      hasStableMac(inc.mac) && historyMap.has(ipOnlyKey)
        ? historyMap.get(ipOnlyKey)!
        : null;
    const isIdentityUpgradeFromIpOnly =
      priorIpOnly !== null && !hasStableMac(priorIpOnly.mac);

    consolidateIpKeyForMacDevice(historyMap, inc);
    const key = deviceHistoryKey(inc.mac, inc.ip);
    const existing = historyMap.get(key);

    const now = Date.now();
    if (existing) {
      historyMap.set(key, {
        ...inc,
        ip: inc.ip,
        customName:
          existing.customName?.trim() ||
          names[inc.ip]?.trim() ||
          inc.customName ||
          null,
        hostname: inc.hostname?.trim() || existing.hostname?.trim() || null,
        mdnsHostname:
          inc.mdnsHostname?.trim() || existing.mdnsHostname?.trim() || null,
        mdnsPrimaryService:
          inc.mdnsPrimaryService?.trim() || existing.mdnsPrimaryService || null,
        name:
          inc.name?.trim() && inc.name !== inc.ip && inc.name.toLowerCase() !== "unknown"
            ? inc.name
            : existing.name || inc.name,
        shieldHighlight: inc.shieldHighlight ?? existing.shieldHighlight,
        likelyType: inc.likelyType?.trim() || existing.likelyType || null,
        ssdpServer: inc.ssdpServer?.trim() || existing.ssdpServer || null,
        interrogationName:
          inc.interrogationName?.trim() || existing.interrogationName || null,
        isNew: false,
        isOnline: true,
        lastSeen: now,
        firstSeen: existing.firstSeen ?? now,
        status: normalizeStatus(inc.status),
      });
    } else if (isIdentityUpgradeFromIpOnly) {
      const prior = priorIpOnly!;
      // Migrate latency samples from the old IP-keyed entry to the stable MAC key.
      migrateLatencyKey(deviceHistoryUniqueId(prior.mac, prior.ip), deviceHistoryUniqueId(inc.mac, inc.ip));
      historyMap.set(key, {
        ...inc,
        ip: inc.ip,
        customName:
          prior.customName?.trim() ||
          names[inc.ip]?.trim() ||
          inc.customName ||
          null,
        hostname: inc.hostname?.trim() || prior.hostname?.trim() || null,
        mdnsHostname:
          inc.mdnsHostname?.trim() || prior.mdnsHostname?.trim() || null,
        mdnsPrimaryService:
          inc.mdnsPrimaryService?.trim() || prior.mdnsPrimaryService || null,
        name:
          inc.name?.trim() && inc.name !== inc.ip && inc.name.toLowerCase() !== "unknown"
            ? inc.name
            : prior.name || inc.name,
        shieldHighlight: inc.shieldHighlight ?? prior.shieldHighlight,
        likelyType: inc.likelyType?.trim() || prior.likelyType || null,
        ssdpServer: inc.ssdpServer?.trim() || prior.ssdpServer || null,
        interrogationName:
          inc.interrogationName?.trim() || prior.interrogationName || null,
        isNew: false,
        isOnline: true,
        lastSeen: now,
        firstSeen: prior.firstSeen ?? now,
        status: normalizeStatus(inc.status),
      });
    } else {
      const nextRow: DeviceRow = {
        ...inc,
        customName: inc.customName?.trim() || names[inc.ip]?.trim() || null,
        isNew: true,
        isOnline: true,
        lastSeen: now,
        firstSeen: now,
        status: normalizeStatus(inc.status),
      };
      historyMap.set(key, nextRow);
      if (hadPriorHistory) {
        newIntruderNotifications.push(nextRow);
      }
    }
  }

  return {
    devices: sortDevicesForDisplay(Array.from(historyMap.values())),
    newIntruderNotifications,
  };
}

/** Fires one OS notification per new host when permission is granted. */
function notifyNewIntruderDevicesIfNeeded(
  newIntruderNotifications: DeviceRow[],
): void {
  if (newIntruderNotifications.length === 0) {
    return;
  }
  if (!isTauri() || typeof window === "undefined") {
    return;
  }
  void (async () => {
    try {
      if (!(await isPermissionGranted())) {
        return;
      }
      for (const device of newIntruderNotifications) {
        const name =
          device.interrogationName?.trim() ||
          device.hostname?.trim() ||
          "Unknown Device";
        sendNotification({
          title: "New Device Detected",
          body: `${name} joined your network (${device.ip}).`,
        });
      }
    } catch (e) {
      console.warn("Intruder notification failed:", e);
    }
  })();
}


/** Drop invalid / ghost entries from disk or in-memory lists before use or persisting. */
function filterValidStoredDeviceRawEntries(
  items: unknown[],
): unknown[] {
  return items.filter(
    (d) =>
      d != null &&
      typeof d === "object" &&
      "ip" in d &&
      typeof (d as { ip: unknown }).ip === "string" &&
      (d as { ip: string }).ip.trim() !== "",
  );
}

function filterValidDevicesForPersistence(devices: DeviceRow[]): DeviceRow[] {
  return devices.filter(
    (d) =>
      d != null &&
      typeof d === "object" &&
      typeof d.ip === "string" &&
      d.ip.trim() !== "",
  );
}

// ── Browser: load from REST API ───────────────────────────────────────────────

const DEVICES_CACHE_KEY = "shabakat_devices_cache_v1";

type ServerDevice = {
  mac: string;
  lastIp?: string | null;
  vendor?: string | null;
  customName?: string | null;
  likelyType?: string | null;
  hostname?: string | null;
  mdnsHostname?: string | null;
  ssdpServer?: string | null;
  interrogationName?: string | null;
  /** Scanner's best computed display name stored by the server after each scan. */
  displayName?: string | null;
  /** Custom mapped icon URL for this device. */
  customIcon?: string | null;
  firstSeen?: number | null;
  lastSeen?: number | null;
  /** True when the device was seen in the most recently completed scan. */
  isOnline?: boolean | null;
};

async function browserLoadDevices(): Promise<DeviceRow[]> {
  try {
    const rows = await invoke<ServerDevice[]>("get_devices");
    console.log("[browserLoadDevices] raw server response:", rows.slice(0, 5));
    const names = readCustomNamesFromStorage();
    const mapped = rows
      .filter((r) => r.mac)
      .map((r): DeviceRow => {
        const ip = r.lastIp ?? "";
        const vendor = r.vendor?.trim() || "";
        const cn = r.customName?.trim() || names[ip]?.trim() || null;
        const displayName =
          cn ||
          r.displayName?.trim() ||
          r.interrogationName?.trim() ||
          r.hostname?.trim() ||
          r.mdnsHostname?.trim() ||
          ip ||
          r.mac;
        const online = r.isOnline ?? false;
        const row: DeviceRow = {
          status: online ? "Online" : "Offline",
          name: displayName,
          ip,
          mac: r.mac,
          vendor,
          vendorName: vendor || "Unknown",
          deviceType: "unknown",
          isRandomized: false,
          mdnsHostname: r.mdnsHostname ?? null,
          mdnsPrimaryService: null,
          likelyType: r.likelyType ?? null,
          interrogationName: r.interrogationName ?? null,
          hostname: r.hostname ?? null,
          ssdpServer: r.ssdpServer ?? null,
          customName: cn,
          displayName: r.displayName?.trim() ?? null,
          customIcon: r.customIcon,
          isOnline: online,
          isNew: false,
          lastSeen: r.lastSeen ?? null,
          firstSeen: r.firstSeen ?? null,
        };
        return row;
      });

    // Cache the fresh results
    try {
      localStorage.setItem(DEVICES_CACHE_KEY, JSON.stringify(mapped));
    } catch (e) {
      console.warn("[browserLoadDevices] Failed to cache devices:", e);
    }

    return mapped;
  } catch (e) {
    console.error("[browserLoadDevices] error:", e);
    // On error, try to return whatever is in cache
    try {
      const cached = localStorage.getItem(DEVICES_CACHE_KEY);
      if (cached) return JSON.parse(cached);
    } catch {}
    return [];
  }
}

async function loadDevicesFromStore(): Promise<DeviceRow[]> {
  if (typeof window === "undefined") return [];
  if (!isTauri()) return browserLoadDevices();
  // ── Tauri: load from plugin-store ────────────────────────────────────────
  try {
    const store = await getStore();
    const raw = await store.get(DEVICES_STORE_KEY);
    if (!Array.isArray(raw)) {
      return [];
    }
    const sanitized = filterValidStoredDeviceRawEntries(raw);
    return sanitized
      .map((item) => normalizeStoredDeviceRow(item))
      .filter((x): x is DeviceRow => x !== null);
  } catch (e) {
    console.error("Failed to load device list from store:", e);
    return [];
  }
}

async function saveDevicesToStore(devices: DeviceRow[]): Promise<void> {
  if (typeof window !== "undefined" && !isTauri()) {
    try {
      localStorage.setItem(DEVICES_CACHE_KEY, JSON.stringify(devices));
    } catch (e) {
      console.warn("[saveDevicesToStore] Failed to cache devices:", e);
    }
    return;
  }
  if (!isTauri() || typeof window === "undefined") {
    return;
  }
  try {
    const store = await getStore();
    const valid = filterValidDevicesForPersistence(devices);
    await store.set(DEVICES_STORE_KEY, valid);
    await store.save();
  } catch (e) {
    console.error("Failed to save device list to store:", e);
  }
}

// Frontend safety timeout is intentionally longer than Rust's internal timeout so
// backend cleanup and guard release happen first on slow networks.

const PERMISSION_DENIED_MESSAGE =
  "Permission required to see MAC addresses.";
const ANDROID_FINE_LOCATION_AR_MESSAGE =
  "يرجى تفعيل إذن الموقع الجغرافي (الدقيق) لرؤية الأجهزة.";
const SCAN_TIMEOUT_MESSAGE =
  "Scan timed out. The network might be too large or restricted.";
const JS_SCAN_TIMEOUT_BY_MODE: Record<ScanMode, number> = {
  // Keep frontend timeout above Rust's 90s cap so backend completes first.
  silent: 140_000,
  aggressive: 120_000,
};
const SCAN_RETRY_ATTEMPTS = 10;
const SCAN_RETRY_DELAY_MS = 500;

function getInvokeErrorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (error && typeof error === "object") {
    const maybeError = (error as { error?: unknown }).error;
    if (typeof maybeError === "string") {
      return maybeError;
    }
    const maybeMessage = (error as { message?: unknown }).message;
    if (typeof maybeMessage === "string") {
      return maybeMessage;
    }
  }
  return String(error ?? "");
}

function isScanInProgressError(error: unknown): boolean {
  return getInvokeErrorMessage(error).trim() === "SCAN_IN_PROGRESS";
}

export type LastScanTelemetry = {
  scanId: string;
  batches: number;
  ipcDevices: number;
  staleBatches: number;
  staleFinishes: number;
  deviceCount: number;
  /** "completed" — scan finished normally; "failed" — Rust emitted scan_failed after scan_started. */
  status: "completed" | "failed";
  /** Reason string from Rust scan_failed payload. Only present when status === "failed". */
  failureReason?: string;
};

export function useNetworkScan() {
  const devices = useDeviceStore((s) => s.devices);
  const [isScanning, setIsScanning] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [progressPct, setProgressPct] = useState(0);
  const [averageLatencyMs, setAverageLatencyMs] = useState<number | null>(null);
  const [scannedHosts, setScannedHosts] = useState(0);
  const [totalHosts, setTotalHosts] = useState(0);
  const [lastScanAt, setLastScanAt] = useState<Date | null>(null);
  const [lastScanTelemetry, setLastScanTelemetry] = useState<LastScanTelemetry | null>(null);
  const [scanPermissionError, setScanPermissionError] = useState<string | null>(
    null,
  );
  const [scanRuntimeError, setScanRuntimeError] = useState<string | null>(null);
  const progressTimer = useRef<number | null>(null);
  const lastProgressEventAtRef = useRef<number>(0);
  /** Deduplicated initial load of `devices.json` + single UI hydration (survives quick scan). */
  const storeHydrationRef = useRef<Promise<void> | null>(null);
  // Synchronous in-flight guard: prevents the TOCTOU race where two rapid taps
  // both pass the `isScanning` React-state check during the async permission
  // handshake gap, causing the second IPC call to hit the Rust ScanGuard and
  // return immediately — which then resets isScanning(false) and lets a 3rd tap
  // through.  useRef is read synchronously, unlike React state.
  const scanInProgressRef = useRef(false);
  /** Scan ID latched from `scan_started`; cleared in finally. Any IPC event with a
   *  different id is definitively stale and is rejected without merging. */
  const activeScanIdRef = useRef<string | null>(null);
  /** FLIGHT_RECORDER: counts good batches and total devices received via IPC. */
  const ipcBatchCountRef = useRef(0);
  const ipcDeviceCountRef = useRef(0);
  /** FLIGHT_RECORDER: counts rejected stale device_discovered / scan_finished events. */
  const staleBatchCountRef = useRef(0);
  const staleFinishCountRef = useRef(0);
  /** Debounce handle for plugin-store persistence. */
  const persistTimerRef = useRef<number | null>(null);
  /** Last accepted batchSeq; reject duplicates / out-of-order deliveries (≤ current). */
  const lastBatchSeqRef = useRef<number>(0);

  const ensureHistoryMapHydrated = useCallback(async () => {
    if (storeHydrationRef.current) {
      await storeHydrationRef.current;
      return;
    }
    if (!isTauri() || typeof window === "undefined") {
      // Browser mode: pre-populate from localStorage (cache) immediately,
      // then fetch from REST API in the background.
      storeHydrationRef.current = (async () => {
        setIsLoading(true);
        // 1. Optimistic load from cache
        try {
          const cached = localStorage.getItem(DEVICES_CACHE_KEY);
          if (cached) {
            const loaded = JSON.parse(cached) as DeviceRow[];
            if (Array.isArray(loaded) && loaded.length > 0) {
              const m = useDeviceStore.getState()._map;
              for (const row of loaded) {
                m.set(deviceHistoryKey(row.mac, row.ip), row);
              }
              useDeviceStore.getState().setDevices(sortDevicesForDisplay(Array.from(m.values())));
            }
          }
        } catch (e) {
          console.warn("[ensureHistoryMapHydrated] Cache load failed:", e);
        }

        // 2. Wait for current scan status to avoid race conditions with backend
        try {
          const status = await invoke<{ isScanning: boolean }>("scan_status");
          if (status.isScanning) {
            console.log("[ensureHistoryMapHydrated] Scan active on server; waiting 2s for stability...");
            await new Promise((resolve) => window.setTimeout(resolve, 2000));
          }
        } catch { /* ignore status check failures */ }

        // 3. Background fetch from server
        try {
          const loaded = await browserLoadDevices();
          const m = useDeviceStore.getState()._map;
          if (loaded.length > 0) {
            // Overwrite map with fresh data from server
            for (const row of loaded) {
              m.set(deviceHistoryKey(row.mac, row.ip), row);
            }
            useDeviceStore.getState().setDevices(sortDevicesForDisplay(Array.from(m.values())));
          }
        } catch {
          // non-fatal: device list starts empty or stays with cached data
        } finally {
          setIsLoading(false);
        }
      })();
      await storeHydrationRef.current;
      return;
    }
    storeHydrationRef.current = (async () => {
      setIsLoading(true);
      try {
        const loaded = await loadDevicesFromStore();
        const m = useDeviceStore.getState()._map;
        m.clear();
        for (const row of loaded) {
          const normalized = normalizeStoredDeviceRow(row);
          if (!normalized) {
            continue;
          }
          m.set(deviceHistoryKey(normalized.mac, normalized.ip), normalized);
        }
        useDeviceStore.getState().setDevices(sortDevicesForDisplay(Array.from(m.values())));
      } finally {
        setIsLoading(false);
      }
    })();
    await storeHydrationRef.current;
  }, []);

  useEffect(() => {
    void ensureHistoryMapHydrated();
  }, [ensureHistoryMapHydrated]);

  const clearProgressTimer = useCallback(() => {
    if (progressTimer.current !== null) {
      window.clearInterval(progressTimer.current);
      progressTimer.current = null;
    }
  }, []);

  useEffect(() => {
    return () => clearProgressTimer();
  }, [clearProgressTimer]);

  // mDNS listener: passive Bonjour/Zeroconf hostname enrichment.
  useEffect(() => {
    if (typeof window === "undefined" || !isTauri()) return;
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      unlisten = await listen<{ ip: string; hostname: string }>(
        "mdns_device_found",
        (event) => {
          const { ip, hostname } = event.payload;
          if (!ip || !hostname) return;
          const m = useDeviceStore.getState()._map;
          let matched = false;
          for (const [key, dev] of m) {
            if (dev.ip !== ip) continue;
            const nameIsDefault =
              !dev.name ||
              dev.name === dev.ip ||
              dev.name === "Unknown Device" ||
              /^\d{1,3}(\.\d{1,3}){3}$/.test(dev.name);
            const updated: DeviceRow = {
              ...dev,
              mdnsHostname: dev.mdnsHostname ?? hostname,
              name: nameIsDefault ? hostname : dev.name,
            };
            m.set(key, updated);
            matched = true;
            console.log(`[mDNS] enriched ${ip} → ${hostname}`);
            break;
          }
          if (!matched) return;
          useDeviceStore.getState().setDevices(sortDevicesForDisplay(Array.from(m.values())));
        },
      );
    };
    void setup();
    return () => { unlisten?.(); };
  }, []); // refs are stable — no deps needed

  const ensurePermissionsForScan = useCallback(async (): Promise<boolean> => {
    console.log("[JS_TRACE] ensurePermissionsForScan:start");
    if (typeof window === "undefined" || !isTauri()) {
      console.log("[JS_TRACE] ensurePermissionsForScan:non-tauri -> true");
      return true;
    }
    setScanPermissionError(null);
    const runningOnAndroid = /\bAndroid\b/i.test(window.navigator.userAgent);
    console.log(`[JS_TRACE] ensurePermissionsForScan:runningOnAndroid=${runningOnAndroid}`);

    // Desktop (macOS/Windows): ARP table reading requires no system permission.
    // Skip the location gate entirely and just ensure notification permission.
    if (!runningOnAndroid) {
      try {
        const notifGranted = await isPermissionGranted();
        if (!notifGranted) { await requestPermission(); }
      } catch (notifErr) {
        console.warn("[JS_TRACE] notification permission:", notifErr);
      }
      return true;
    }

    try {
      if (runningOnAndroid) {
        // Backend-side Android permission probe before starting the scan flow.
        console.log("[JS_TRACE] invoke request_android_permissions (preflight)");
        const preflight = await invoke<AndroidPermissionResult>(
          "request_android_permissions",
        );
        console.log("[JS_TRACE] preflight result:", preflight);
        if (preflight.status !== "granted") {
          console.log("[JS_TRACE] requesting geolocation permission from plugin");
          await requestPermissions(["location"]);
        }
      }

      const status = await checkPermissions();
      console.log("[JS_TRACE] checkPermissions result:", status);
      const locationGranted = status.location === "granted";
      let fineLocationGranted = true;
      let androidPermissionBundleGranted = true;
      if (runningOnAndroid) {
        console.log("[JS_TRACE] invoke request_android_permissions (post-check)");
        const androidPermissionResult = await invoke<AndroidPermissionResult>(
          "request_android_permissions",
        );
        console.log("[JS_TRACE] post-check permission bundle:", androidPermissionResult);
        androidPermissionBundleGranted = androidPermissionResult.status === "granted";
        console.log("[JS_TRACE] invoke check_permission for ACCESS_FINE_LOCATION");
        fineLocationGranted = await invoke<boolean>("check_permission", {
          permission: "android.permission.ACCESS_FINE_LOCATION",
        });
        console.log(`[JS_TRACE] fineLocationGranted=${fineLocationGranted}`);
      }
      const ok = locationGranted && fineLocationGranted && androidPermissionBundleGranted;
      console.log(
        `[JS_TRACE] permission gate result: locationGranted=${locationGranted}, fineLocationGranted=${fineLocationGranted}, androidBundle=${androidPermissionBundleGranted}, ok=${ok}`,
      );
      try {
        let notifGranted = await isPermissionGranted();
        if (!notifGranted) {
          await requestPermission();
        }
      } catch (notifErr) {
        console.warn("[JS_TRACE] notification permission:", notifErr);
      }
      if (!ok) {
        const message = runningOnAndroid
          ? ANDROID_FINE_LOCATION_AR_MESSAGE
          : PERMISSION_DENIED_MESSAGE;
        setScanPermissionError(message);
        if (runningOnAndroid) {
          window.alert(ANDROID_FINE_LOCATION_AR_MESSAGE);
        }
      }
      return ok;
    } catch (error) {
      console.error("[JS_TRACE] Permission handshake failed (raw):", error);
      console.error("[JS_TRACE] Permission handshake failed (stringified):", {
        name: error instanceof Error ? error.name : undefined,
        message: error instanceof Error ? error.message : String(error),
        stack: error instanceof Error ? error.stack : undefined,
      });
      setScanPermissionError(PERMISSION_DENIED_MESSAGE);
      return false;
    }
  }, []);

  const triggerScan = useCallback(async (mode: ScanMode = "silent") => {
    // Purge any zombie Rust thread before evaluating guards.
    // cancel_scan is a sync Tauri command so it bypasses the tokio worker pool.
    try { await invoke("cancel_scan"); } catch { /* ignore — no scan running */ }

    console.log(`[FLIGHT_RECORDER] Frontend state: isScanning=${isScanning}`);
    if (isScanning) {
      return;
    }
    // Ref guard is checked synchronously — React state updates are batched and
    // isScanning may be stale across async gaps.
    if (scanInProgressRef.current) {
      console.log("[LOCK] A scan is already running. Ignoring this request.");
      return;
    }
    scanInProgressRef.current = true;
    // Reset stale-batch guard and IPC flight-recorder counters for this scan.
    activeScanIdRef.current = null;
    ipcBatchCountRef.current = 0;
    ipcDeviceCountRef.current = 0;
    staleBatchCountRef.current = 0;
    staleFinishCountRef.current = 0;
    lastBatchSeqRef.current = 0;

    setProgressPct(0);
    setIsScanning(true);
    setScannedHosts(0);
    setTotalHosts(0);
    setScanRuntimeError(null);
    lastProgressEventAtRef.current = Date.now();
    clearProgressTimer();

    let unlistenScanStarted: (() => void) | undefined;
    let unlistenScanFailed: (() => void) | undefined;
    let unlistenDeviceDiscovered: (() => void) | undefined;
    let unlistenScanFinished: (() => void) | undefined;
    let unlistenScanReset: (() => void) | undefined;

    try {
      // Step A: geolocation handshake — Rust must not run until this resolves granted.
      console.log("[SCAN_TRACE] Requesting permissions...");
      const permissionOk = await ensurePermissionsForScan();
      if (!permissionOk) {
        console.warn("[SCAN_TRACE] Permission denied — scan aborted");
        return; // finally resets scanInProgressRef and isScanning
      }

      // Must not run a pass until the persistent map is filled; otherwise every host looks "new".
      await ensureHistoryMapHydrated();

      // Step B: kick off progress animation and invoke scan.
      console.log("[SCAN_TRACE] Permissions OK, invoking Rust scan...");
      clearProgressTimer();
      progressTimer.current = window.setInterval(() => {
        setProgressPct((prev) => {
          // Quiet-mode scan can take longer on large networks; keep the progress
          // moving slowly and steadily to show liveliness.
          if (prev < 95) return prev + 1;
          return prev;
        });
      }, 750);

      if (typeof window !== "undefined") {
        // scan_started MUST be registered first — Rust emits it at the very top of
        // scan_network before any discovery work, so this handler runs and latches
        // activeScanIdRef before the first device_discovered arrives.
        unlistenScanStarted = await listen<ScanStartedPayload>(
          "scan_started",
          (event) => {
            activeScanIdRef.current = event.payload.scanId;
            lastBatchSeqRef.current = 0;
            console.log(`[SCAN_TRACE] scan_started — activeScanId latched as ${event.payload.scanId}`);
          },
        );

        unlistenScanFailed = await listen<{ scanId: string; reason: string }>(
          "scan_failed",
          (event) => {
            console.warn("[SCAN_LIFECYCLE] scan_failed received", event.payload);
            if (event.payload.scanId === activeScanIdRef.current) {
              setLastScanTelemetry({
                scanId: event.payload.scanId,
                batches: ipcBatchCountRef.current,
                ipcDevices: ipcDeviceCountRef.current,
                staleBatches: staleBatchCountRef.current,
                staleFinishes: staleFinishCountRef.current,
                deviceCount: useDeviceStore.getState().devices.length,
                status: "failed",
                failureReason: event.payload.reason,
              });
              activeScanIdRef.current = null;
            }
          },
        );

        unlistenScanFinished = await listen<ScanNetworkPayload>(
          "scan_finished",
          (event) => {
            // Strict stale guard: activeScanIdRef is always set from scan_started before
            // this fires, so any mismatch is definitively from a prior scan.
            if (event.payload.scanId !== activeScanIdRef.current) {
              staleFinishCountRef.current += 1;
              console.warn("[STALE_FINISH]", {
                activeScanId: activeScanIdRef.current,
                incomingScanId: event.payload.scanId,
              });
              return;
            }
            console.log(`[SCAN_TRACE] scan_finished | scanId=${event.payload.scanId} | batches=${ipcBatchCountRef.current} | devices=${ipcDeviceCountRef.current}`);
            setAverageLatencyMs(event.payload.averageLatencyMs ?? null);
            setScannedHosts(event.payload.scannedHosts ?? 0);
            setTotalHosts(event.payload.totalHosts ?? 0);
            setProgressPct(100);
            const payloadDevices = event.payload?.devices;
            const { _map: historyMap, setDevices } = useDeviceStore.getState();
            if (Array.isArray(payloadDevices) && payloadDevices.length > 0) {
              const names = readCustomNamesFromStorage();
              const rows = payloadDevices.map((p) => mapDiscoveredToRow(p, names));
              const merged = mergeScanProgress(historyMap, rows);
              notifyNewIntruderDevicesIfNeeded(merged.newIntruderNotifications);
              setDevices(merged.devices);
            } else {
              setDevices(sortDevicesForDisplay(Array.from(historyMap.values())));
            }
            // Persistence is handled once in the post-invoke path; skip here to
            // avoid a duplicate write racing the command-resolve save.
          },
        );

        unlistenDeviceDiscovered = await listen<ScanNetworkPayload>(
          "device_discovered",
          (event) => {
            lastProgressEventAtRef.current = Date.now();
            const payloadDevices = event.payload?.devices;
            if (!Array.isArray(payloadDevices) || payloadDevices.length === 0) {
              return;
            }

            // Strict stale guard: activeScanIdRef is set from scan_started, so any
            // event with a different id is from a prior scan and must be rejected.
            if (event.payload.scanId !== activeScanIdRef.current) {
              staleBatchCountRef.current += 1;
              console.warn("[STALE_BATCH]", {
                activeScanId: activeScanIdRef.current,
                incomingScanId: event.payload.scanId,
              });
              return;
            }

            // Reject duplicate or out-of-order batches from IPC replay / redelivery.
            const seq = event.payload.batchSeq ?? 0;
            if (seq > 0 && seq <= lastBatchSeqRef.current) {
              console.warn("[STALE_BATCH_SEQ]", { batchSeq: seq, lastSeq: lastBatchSeqRef.current });
              return;
            }
            lastBatchSeqRef.current = seq;

            ipcBatchCountRef.current += 1;
            ipcDeviceCountRef.current += payloadDevices.length;
            console.log(
              `[FLIGHT_RECORDER] batch #${ipcBatchCountRef.current} seq=${event.payload.batchSeq ?? "?"} | ${payloadDevices.length} device(s) | total: ${ipcDeviceCountRef.current}`,
            );
            const first = payloadDevices[0];
            if (first) {
              console.log(
                "[SCAN_TRACE_TYPE] React Received | IP:",
                first.ip,
                "likelyType:",
                first.likelyType ?? "(null/undefined)",
              );
            }
            const names = readCustomNamesFromStorage();
            const rows = payloadDevices.map((p) => mapDiscoveredToRow(p, names));
            setScannedHosts(event.payload.scannedHosts ?? 0);
            setTotalHosts(event.payload.totalHosts ?? 0);
            const { _map: historyMap, setDevices } = useDeviceStore.getState();
            const merged = mergeScanProgress(historyMap, rows);
            notifyNewIntruderDevicesIfNeeded(merged.newIntruderNotifications);
            setDevices(merged.devices);
            setProgressPct((prev) => Math.min(prev + 1, 96));
          },
        );

        unlistenScanReset = await listen<ScanNetworkPayload>(
          "network-scan-reset",
          () => {
            useDeviceStore.getState().resetStore();
            setScannedHosts(0);
            setTotalHosts(0);
          },
        );
      }

      // Mark remembered hosts offline for this pass (do not clear history).
      {
        const { _map: historyMap, setDevices } = useDeviceStore.getState();
        markHistoryOffline(historyMap);
        setDevices(sortDevicesForDisplay(Array.from(historyMap.values())));
      }

      // Await backend result with an additional frontend safety valve. isScanning
      // is set to false exclusively in the finally block below, never here,
      // ensuring state integrity on every exit path.
      let jsSafetyTimeoutId: number | null = null;
      let result: ScanNetworkPayload | null = null;
      console.log("[SCAN_TRACE] Scanning large network, increasing wait time...");
      for (let attempt = 1; attempt <= SCAN_RETRY_ATTEMPTS; attempt += 1) {
        try {
          const jsSafetyTimeout = new Promise<never>((_, reject) => {
            jsSafetyTimeoutId = window.setTimeout(() => {
              reject(new Error(SCAN_TIMEOUT_MESSAGE));
            }, JS_SCAN_TIMEOUT_BY_MODE[mode]);
          });
          result = (await Promise.race([
            invoke<ScanNetworkPayload>("scan_network", { mode }),
            jsSafetyTimeout,
          ])) as ScanNetworkPayload;
          break;
        } catch (error) {
          if (!isScanInProgressError(error)) {
            throw error;
          }
          if (attempt >= SCAN_RETRY_ATTEMPTS) {
            throw new Error("SCAN_IN_PROGRESS");
          }
          console.log("[JS_TRACE] Waiting for previous scan to release lock...");
          await new Promise<void>((r) =>
            window.setTimeout(r, SCAN_RETRY_DELAY_MS),
          );
        } finally {
          if (jsSafetyTimeoutId !== null) {
            window.clearTimeout(jsSafetyTimeoutId);
            jsSafetyTimeoutId = null;
          }
        }
      }
      if (!result) {
        throw new Error("SCAN_IN_PROGRESS");
      }
      console.log("[FLIGHT_RECORDER] Rust returned raw data:", result);
      console.log("[SCAN_TRACE] Rust scan invoke resolved (streaming mode)");
      // Final sync: merge the full device list returned by the command so any
      // device_discovered events that were dropped or reordered are recovered.
      const commandDevices = result?.devices;
      {
        const { _map: historyMap, setDevices } = useDeviceStore.getState();
        if (result && Array.isArray(commandDevices) && commandDevices.length > 0) {
          const names = readCustomNamesFromStorage();
          const rows = commandDevices.map((p) => mapDiscoveredToRow(p, names));
          const merged = mergeScanProgress(historyMap, rows);
          notifyNewIntruderDevicesIfNeeded(merged.newIntruderNotifications);
          setDevices(merged.devices);
        } else {
          setDevices(sortDevicesForDisplay(Array.from(historyMap.values())));
        }
      }
      // Debounced persist: cancel any prior timer and schedule a single write
      // 1.5 s after the scan completes so rapid back-to-back scans coalesce.
      if (persistTimerRef.current !== null) window.clearTimeout(persistTimerRef.current);
      persistTimerRef.current = window.setTimeout(() => {
        void saveDevicesToStore(useDeviceStore.getState().devices);
        persistTimerRef.current = null;
      }, 1500);
      setLastScanAt(new Date());
    } catch (err) {
      // scan_network rejected (Rust error, ScanGuard locked, network unavailable, etc.).
      console.error("[SCAN_TRACE] Rust scan rejected (raw):", err);
      console.error("[SCAN_TRACE] Rust scan rejected (details):", {
        name: err instanceof Error ? err.name : undefined,
        message: err instanceof Error ? err.message : String(err ?? "Scan failed"),
        stack: err instanceof Error ? err.stack : undefined,
        rawError: err,
      });
      const message =
        err instanceof Error ? err.message : String(err ?? "Scan failed");
      const isTimeout = message.toLowerCase().includes("timed out");
      if (isTimeout) {
        setScanRuntimeError(SCAN_TIMEOUT_MESSAGE);
      }
      void saveDevicesToStore(useDeviceStore.getState().devices);
      setAverageLatencyMs(null);
      setScannedHosts(0);
      setTotalHosts(0);
      setProgressPct(0);
    } finally {
      const finalDeviceCount = useDeviceStore.getState().devices.length;
      const finalScanId = activeScanIdRef.current ?? "?";
      console.log(
        `[FLIGHT_RECORDER] Scan complete | devices=${finalDeviceCount} | batches=${ipcBatchCountRef.current} | ipcDevices=${ipcDeviceCountRef.current} | staleBatches=${staleBatchCountRef.current} | staleFinishes=${staleFinishCountRef.current} | scanId=${finalScanId}`,
      );
      if (finalScanId !== "?") {
        setLastScanTelemetry({
          scanId: finalScanId,
          batches: ipcBatchCountRef.current,
          ipcDevices: ipcDeviceCountRef.current,
          staleBatches: staleBatchCountRef.current,
          staleFinishes: staleFinishCountRef.current,
          deviceCount: finalDeviceCount,
          status: "completed",
        });
      }
      evictLatencyCache();
      activeScanIdRef.current = null;
      scanInProgressRef.current = false;
      setIsScanning(false);
      console.log("[FLIGHT_RECORDER] UI Lock released.");
      unlistenScanStarted?.();
      unlistenScanFailed?.();
      unlistenScanFinished?.();
      unlistenDeviceDiscovered?.();
      unlistenScanReset?.();
      clearProgressTimer();
      // Cancel any pending debounce timer from a mid-scan persist schedule.
      if (persistTimerRef.current !== null) {
        window.clearTimeout(persistTimerRef.current);
        persistTimerRef.current = null;
      }
    }
    // isScanning intentionally omitted from deps — the ref is the synchronous guard.
  }, [clearProgressTimer, ensureHistoryMapHydrated, ensurePermissionsForScan]);

  const cancelScan = useCallback(async () => {
    if (!isTauri()) {
      return;
    }
    try {
      await invoke("abort_scan");
    } catch (error) {
      console.error("Failed to abort scan:", error);
      try {
        await invoke("cancel_scan");
      } catch (fallbackError) {
        console.error("Fallback cancel_scan failed:", fallbackError);
      }
    }
  }, []);

  const patchDevice = useCallback(
    (ip: string, patch: Partial<DeviceRow>) => {
      useDeviceStore.getState().patchDevice(ip, patch);
    },
    [],
  );

  return {
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
    /** Same as standalone `ensurePermissions`, but clears/sets `scanPermissionError` for UI. */
    ensurePermissions: ensurePermissionsForScan,
    scanPermissionError,
    scanRuntimeError,
    clearScanRuntimeError: () => setScanRuntimeError(null),
    clearScanPermissionError: () => setScanPermissionError(null),
  };
}

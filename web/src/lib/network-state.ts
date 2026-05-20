import { invoke, isTauri } from "@/lib/transport";

export type ConnectionType =
  | "wifi"
  | "ethernet"
  | "cellular"
  | "none"
  | "unknown";

export type NetworkState = {
  isOnline: boolean;
  connectionType: ConnectionType;
};

/** Subset of the Network Information API available in Chromium WebViews. */
type NavigatorConnection = {
  readonly type?:
    | "bluetooth"
    | "cellular"
    | "ethernet"
    | "none"
    | "wifi"
    | "wimax"
    | "other"
    | "unknown";
};

function navConnection(): NavigatorConnection | undefined {
  if (typeof navigator === "undefined") {
    return undefined;
  }
  const n = navigator as Navigator & {
    connection?: NavigatorConnection;
    mozConnection?: NavigatorConnection;
    webkitConnection?: NavigatorConnection;
  };
  return n.connection ?? n.mozConnection ?? n.webkitConnection;
}

function inferFromNavigatorConnection(): ConnectionType | null {
  const raw = navConnection()?.type;
  switch (raw) {
    case "wifi":
      return "wifi";
    case "ethernet":
      return "ethernet";
    case "cellular":
    case "wimax":
      return "cellular";
    case "none":
      return "none";
    case "bluetooth":
    case "unknown":
    case "other":
    case undefined:
      return null;
    default:
      return null;
  }
}

/** Returns true for all RFC-1918 private IPv4 ranges: 10.x.x.x, 172.16–31.x.x, 192.168.x.x. */
function isPrivateLan(ip: string): boolean {
  const t = ip.trim();
  return (
    /^10\.\d{1,3}\.\d{1,3}\.\d{1,3}$/u.test(t) ||
    /^172\.(1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}$/u.test(t) ||
    /^192\.168\.\d{1,3}\.\d{1,3}$/u.test(t)
  );
}

/**
 * LAN discovery (ARP / multicast) should only run on Wi‑Fi / Ethernet —
 * never over cellular — so we expose this flag for Radar / scanner entry points.
 */
export function lanScanAllowedForState(state: NetworkState): boolean {
  if (!state.isOnline) {
    return false;
  }
  return (
    state.connectionType === "wifi" || state.connectionType === "ethernet"
  );
}

export async function getNetworkState(): Promise<NetworkState> {
  const browserOnline =
    typeof navigator !== "undefined" ? navigator.onLine : true;

  let connectionType = inferFromNavigatorConnection();

  if (isTauri()) {
    try {
      if (!browserOnline) {
        return { isOnline: false, connectionType: "none" };
      }
      const activeIp = await invoke<string>("get_active_ip");
      if (activeIp) {
        connectionType ??= isPrivateLan(activeIp) ? "wifi" : "unknown";
      }
    } catch {
      /* fall through to navigator-only */
    }
    connectionType = "wifi";
    return { isOnline: browserOnline, connectionType: "wifi" };
  }

  if (!browserOnline) {
    return { isOnline: false, connectionType: "none" };
  }

  return {
    isOnline: true,
    connectionType: connectionType ?? "unknown",
  };
}

export type NetworkStateListenerOptions = {
  /** When false, skips the initial `getNetworkState()` push (callers often await `getNetworkState()` first). @default true */
  emitInitial?: boolean;
};

/**
 * Emits updates when connectivity changes (online/offline, Network Information API,
 * periodic interface poll for Wi‑Fi ↔ cellular switches that don't toggle `onLine`).
 */
export async function onNetworkStateChanged(
  handler: (state: NetworkState) => void,
  options?: NetworkStateListenerOptions,
): Promise<() => void> {
  const emitInitial = options?.emitInitial ?? true;

  const notify = async () => {
    handler(await getNetworkState());
  };

  if (emitInitial) {
    await notify();
  }

  const onOnline = () => void notify();
  const onOffline = () => void notify();

  window.addEventListener("online", onOnline);
  window.addEventListener("offline", onOffline);

  const conn = navConnection();
  const onConnChange = () => void notify();
  (conn as unknown as EventTarget | undefined)?.addEventListener?.(
    "change",
    onConnChange,
  );

  return () => {
    window.removeEventListener("online", onOnline);
    window.removeEventListener("offline", onOffline);
    (conn as unknown as EventTarget | undefined)?.removeEventListener?.(
      "change",
      onConnChange,
    );
  };
}

/**
 * Transport adapter — Tauri IPC in-app, HTTP + WebSocket in browser.
 * Import { invoke, listen, isTauri } from here instead of @tauri-apps/api/*.
 */
import {
  invoke as tauriInvoke,
  isTauri as tauriIsTauri,
} from "@tauri-apps/api/core";
import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import { listen as tauriListen } from "@tauri-apps/api/event";

export { tauriIsTauri as isTauri };
export type { UnlistenFn, EventCallback };

// ── WebSocket singleton ───────────────────────────────────────────────────────

let _ws: WebSocket | null = null;
let _reconnectAttempts = 0;
const MAX_RECONNECT_DELAY = 30000;
const _handlers = new Map<string, Array<(data: unknown) => void>>();

/** Dispatch an event directly to local handlers (browser-mode simulation). */
function localDispatch(event: string, data: unknown): void {
  const hs = _handlers.get(event);
  if (hs) for (const h of [...hs]) h(data);
}

// Interval handles for client-side latency polling (browser mode).
const _latencyTimers = new Map<string, number>();

function parsePingLatency(raw: string): number | null {
  const m = /(?:rtt|round-trip)\s+\S+\s*=\s*[\d.]+\/([\d.]+)\//.exec(raw);
  if (m) return parseFloat(m[1]);
  const m2 = /time[=<]\s*([\d.]+)\s*ms/i.exec(raw);
  if (m2) return parseFloat(m2[1]);
  return null;
}

function ensureWs(): void {
  if (_ws && (_ws.readyState === WebSocket.OPEN || _ws.readyState === WebSocket.CONNECTING)) return;
  
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${proto}//${location.host}/ws`);
  
  ws.onopen = () => {
    _reconnectAttempts = 0;
  };

  ws.onmessage = (e) => {
    try {
      const msg = JSON.parse(e.data as string) as { event: string; data: unknown };
      const hs = _handlers.get(msg.event);
      if (hs) for (const h of [...hs]) h(msg.data);
    } catch { /* ignore malformed */ }
  };

  ws.onclose = () => {
    _ws = null;
    const delay = Math.min(1000 * Math.pow(2, _reconnectAttempts), MAX_RECONNECT_DELAY);
    _reconnectAttempts++;
    setTimeout(ensureWs, delay);
  };

  ws.onerror = () => {
    ws.close();
  };

  _ws = ws;
}

function wsListen<T>(event: string, handler: (data: T) => void): UnlistenFn {
  ensureWs();
  const wrapped = (data: unknown) => handler(data as T);
  let arr = _handlers.get(event);
  if (!arr) { arr = []; _handlers.set(event, arr); }
  arr.push(wrapped);
  return () => {
    const a = _handlers.get(event);
    if (!a) return;
    const i = a.indexOf(wrapped);
    if (i >= 0) a.splice(i, 1);
    if (!a.length) _handlers.delete(event);
  };
}

// Eagerly connect the WebSocket when this module is imported in browser mode
// so it's ready before the user clicks Scan.
if (typeof window !== "undefined" && !tauriIsTauri()) {
  ensureWs();
}

// ── listen() ─────────────────────────────────────────────────────────────────

export function listen<T>(
  event: string,
  handler: EventCallback<T>,
): Promise<UnlistenFn> {
  if (tauriIsTauri()) return tauriListen<T>(event, handler);
  const unlisten = wsListen<T>(event, (data) =>
    handler({ event, payload: data, id: 0 }),
  );
  return Promise.resolve(unlisten);
}

// ── Command map ───────────────────────────────────────────────────────────────

type ScanFinishedData = {
  scanId: string;
  averageLatencyMs: number | null;
  scannedHosts: number;
  devices?: any[];
};

/**
 * Helper for browser-mode REST calls.
 * Handles 401/500 gracefully by returning an error object instead of throwing,
 * and ensures Content-Type: application/json is set for POST/PATCH/PUT.
 */
async function browserRequest<T>(
  url: string,
  init?: RequestInit
): Promise<T> {
  const method = init?.method?.toUpperCase() || "GET";
  const headers = new Headers(init?.headers);

  // Auto-set Content-Type for JSON payloads
  if (["POST", "PATCH", "PUT"].includes(method)) {
    if (!headers.has("Content-Type") && !(init?.body instanceof FormData)) {
      headers.set("Content-Type", "application/json");
    }
  }

  try {
    const res = await fetch(url, {
      ...init,
      headers,
      credentials: "include",
    });

    if (!res.ok) {
      const errorText = await res.text().catch(() => "Unknown error");
      let errorObj: any;
      try {
        errorObj = JSON.parse(errorText);
      } catch {
        errorObj = { error: errorText || `HTTP ${res.status}` };
      }

      console.error(`[API Error] ${method} ${url} ${res.status}:`, errorObj);
      
      // Return error object as T so callers get it instead of a crash/throw
      return errorObj as unknown as T;
    }

    if (res.status === 204) return undefined as unknown as T;
    return res.json() as Promise<T>;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`[Network Error] ${method} ${url}:`, msg);
    return { error: msg } as unknown as T;
  }
}

async function browserInvoke<T>(command: string, args: Record<string, unknown>): Promise<T> {
  // No-ops: Android/permission commands that don't apply in browser
  if (command === "cancel_scan" || command === "abort_scan") return undefined as unknown as T;
  if (command === "get_active_ip") return "" as unknown as T;
  if (command === "request_android_permissions") {
    return { status: "granted", fineLocation: true, nearbyWifiDevices: true, coarseLocation: true } as unknown as T;
  }
  if (command === "check_permission") return true as unknown as T;

  // Scan: POST /api/scan, then wait for scan_finished on WebSocket
  if (command === "scan_network") {
    const res = await browserRequest<any>("/api/scan", {
      method: "POST",
      body: JSON.stringify({ mode: args.mode ?? "silent" }),
    });
    
    // We still throw for the scan engine if it's a known conflict, 
    // as useNetworkScan.ts expects to catch these specific strings.
    if (res.error === "SCAN_IN_PROGRESS") throw new Error("SCAN_IN_PROGRESS");
    if (res.error) throw new Error(res.error);

    const { scanId } = res as { scanId: string };

    return new Promise<T>((resolve, reject) => {
      const tid = window.setTimeout(() => {
        unlisten();
        reject(new Error("Scan timed out."));
      }, 140_000);
      const unlisten = wsListen<ScanFinishedData>("scan_finished", (data) => {
        if (data.scanId !== scanId) return;
        window.clearTimeout(tid);
        unlisten();
        resolve({
          devices: data.devices ?? [],
          scanId: data.scanId,
          averageLatencyMs: data.averageLatencyMs ?? null,
          scannedHosts: data.scannedHosts ?? 0,
          totalHosts: 0,
        } as unknown as T);
      });
    });
  }

  // Deep port scan → REST + synthetic progress event
  if (command === "run_deep_scan") {
    const ip = ((args.ip as string) ?? "").trim();
    console.log("[DeepScan] browser mode — POST /api/tools/portscan, ip:", ip);
    const res = await browserRequest<any>("/api/tools/portscan", {
      method: "POST",
      body: JSON.stringify({ ip }),
    });
    if (res.error) throw new Error(res.error);

    const openPorts = (res.openPorts as number[]) ?? [];
    const PORTSCAN_TOTAL = 14; // matches SCAN_PORTS count in tools.rs
    localDispatch("deep_scan_progress", { ip, totalPorts: PORTSCAN_TOTAL, portsChecked: PORTSCAN_TOTAL, openPorts });
    return undefined as unknown as T;
  }

  // Latency stream: client-side ping poll that dispatches latency_update events
  if (command === "start_latency_stream") {
    const ip = ((args.ip as string) ?? "").trim();
    if (!ip || _latencyTimers.has(ip)) return undefined as unknown as T;
    const poll = async () => {
      try {
        const res = await browserRequest<any>("/api/tools/ping", {
          method: "POST",
          body: JSON.stringify({ ip }),
        });
        if (!res.error) {
          const latencyMs = parsePingLatency(res as string);
          localDispatch("latency_update", { ip, latencyMs });
        }
      } catch { /* ignore */ }
    };
    void poll(); // fire immediately
    const id = window.setInterval(poll, 6000); // 6s: safe gap for 4-ping rounds
    _latencyTimers.set(ip, id);
    return undefined as unknown as T;
  }

  if (command === "stop_latency_stream") {
    const ip = ((args.ip as string) ?? "").trim();
    if (ip && _latencyTimers.has(ip)) {
      window.clearInterval(_latencyTimers.get(ip)!);
      _latencyTimers.delete(ip);
    } else if (!ip) {
      for (const id of _latencyTimers.values()) window.clearInterval(id);
      _latencyTimers.clear();
    }
    return undefined as unknown as T;
  }

  // New-device events (Alerts page)
  if (command === "get_new_device_events") {
    const res = await browserRequest<any[]>("/api/events");
    if ((res as any).error) throw new Error((res as any).error);

    type RawEvent = {
      id: number;
      eventType: string;
      timestamp: number;
      details: string | null;
      mac: string | null;
    };
    return (res as unknown as RawEvent[])
      .filter((e) => e.eventType === "new_device")
      .map((e) => {
        let ip = "";
        let name = e.mac ?? "Unknown Device";
        try {
          if (e.details) {
            const d = JSON.parse(e.details) as {
              ip?: string;
              mac?: string;
              vendor?: string;
            };
            ip = d.ip ?? "";
            name =
              d.vendor && d.vendor !== "Unknown"
                ? `${d.vendor} Device`
                : (d.mac ?? name);
          }
        } catch {
          /* ignore */
        }
        return { timestampMs: e.timestamp, mac: e.mac ?? "", name, ip };
      }) as unknown as T;
  }

  // Acknowledge device — PATCH /api/devices/:mac
  if (command === "acknowledge_device") {
    const mac = ((args.mac as string) ?? "").trim();
    if (!mac) return undefined as unknown as T;
    return browserRequest<T>(`/api/devices/${encodeURIComponent(mac)}`, {
      method: "PATCH",
      body: JSON.stringify({ acknowledged: true }),
    });
  }

  // Standard REST mappings
  if (command === "run_speed_test")          return browserRequest<T>("/api/speed-test/run", { method: "POST" });
  if (command === "get_speed_test_history")   return browserRequest<T>("/api/speed-test/history");
  if (command === "get_network_info")         return browserRequest<T>("/api/network-info");
  if (command === "get_wifi_info")            return browserRequest<T>("/api/network-info");
  if (command === "get_networks")             return browserRequest<T>("/api/networks");
  if (command === "get_outages")              return browserRequest<T>("/api/outages");
  if (command === "get_devices")               return browserRequest<T>("/api/devices");
  if (command === "get_events")                return browserRequest<T>("/api/events");
  if (command === "get_system_status")         return browserRequest<T>("/api/system-status");

  if (command === "get_device_dns_stats") {
    return browserRequest<T>(`/api/devices/${args.ip}/dns`);
  }
  if (command === "get_device_history") {
    const mac = args.mac as string;
    const limit = args.limit ?? 20;
    return browserRequest<T>(`/api/devices/${encodeURIComponent(mac)}/history?limit=${limit}`);
  }
  if (command === "get_history") {
    const limit = args.limit ?? 300;
    return browserRequest<T>(`/api/history?limit=${limit}`);
  }

  // Router bandwidth stats → REST
  if (command === "get_local_link_stats") {
    const res = await browserRequest<any>("/api/router/bandwidth");
    if (res.error) return { interfaceName: "Router", connectionType: "WAN", liveRxBytes: 0, liveTxBytes: 0 } as unknown as T;
    return {
      interfaceName: "Router",
      connectionType: "WAN",
      liveRxBytes: res.rxBytes ?? 0,
      liveTxBytes: res.txBytes ?? 0,
    } as unknown as T;
  }

  // Tool commands → REST
  const TOOL_MAP: Record<string, { path: string; body: (a: Record<string, unknown>) => unknown }> = {
    ping_device:       { path: "/api/tools/ping",        body: (a) => ({ ip: a.target }) },
    tcp_ping:          { path: "/api/tools/tcp-ping",    body: (a) => ({ ip: a.target, port: a.port }) },
    dns_lookup:        { path: "/api/tools/dns",         body: (a) => ({ target: a.target }) },
    wake_on_lan:       { path: "/api/tools/wake",        body: (a) => ({ mac: a.macAddress }) },
    wake_device:       { path: "/api/tools/wake",        body: (a) => ({ mac: a.macAddress }) },
    scan_device_ports: { path: "/api/tools/portscan",    body: (a) => ({ ip: a.ip }) },
    scan_all_device_ports: { path: "/api/tools/portscan-all", body: (a) => ({ ips: a.ips }) },
    subnet_calc:       { path: "/api/tools/subnet-calc", body: (a) => ({ cidr: a.cidr }) },
    ssl_lookup:        { path: "/api/tools/ssl",         body: (a) => ({ domain: a.domain }) },
    whois_lookup:      { path: "/api/tools/whois",       body: (a) => ({ domain: a.domain }) },
    ip_geolocation:    { path: "/api/tools/ip-geo",      body: (a) => ({ ip: a.ip ?? null }) },
    mac_lookup:        { path: "/api/tools/mac-lookup",  body: (a) => ({ mac: a.mac }) },
    analyze_headers:   { path: "/api/tools/headers",     body: (a) => ({ url: a.url }) },
  };

  const def = TOOL_MAP[command];
  if (!def) throw new Error(`No browser mapping for Tauri command: ${command}`);

  return browserRequest<T>(def.path, {
    method: "POST",
    body: JSON.stringify(def.body(args)),
  });
}

// ── invoke() — public API ────────────────────────────────────────────────────

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (tauriIsTauri()) return tauriInvoke<T>(command, args);
  return browserInvoke<T>(command, args ?? {});
}

export const transport = {
  /**
   * Standard fetch wrapper. Ensures credentials: 'include' and 
   * auto-sets Content-Type: application/json for POST/PATCH/PUT if body isn't FormData.
   */
  fetch: (input: RequestInfo | URL, init?: RequestInit) => {
    const options: RequestInit = { 
      credentials: "include",
      ...init 
    };
    const method = (options.method || "GET").toUpperCase();
    if (["POST", "PATCH", "PUT"].includes(method)) {
      const headers = new Headers(options.headers);
      if (!headers.has("Content-Type") && !(options.body instanceof FormData)) {
        headers.set("Content-Type", "application/json");
      }
      options.headers = headers;
    }
    return window.fetch(input, options);
  },
  invoke,
  isTauri: tauriIsTauri,
};

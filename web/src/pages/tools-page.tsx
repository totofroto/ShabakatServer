import { useEffect, useRef, useState } from "react";
import { invoke } from "@/lib/transport";
import { listen } from "@/lib/transport";
import {
  Activity,
  BookOpen,
  ChevronLeft,
  Cpu,
  FileCode,
  Globe,
  Lock,
  MapPin,
  Network,
  Power,
  ScanSearch,
  Terminal,
  Zap,
} from "lucide-react";
import { PingTool } from "@/components/tools/PingTool";
import { PortScannerTool } from "@/components/tools/PortScannerTool";
import { DnsLookupTool } from "@/components/tools/DnsLookupTool";
import { MacLookupTool } from "@/components/tools/MacLookupTool";
import { WolTool } from "@/components/tools/WolTool";
import { IpGeoTool } from "@/components/tools/IpGeoTool";
import { SubnetCalcTool } from "@/components/tools/SubnetCalcTool";
import { HttpHeaderTool } from "@/components/tools/HttpHeaderTool";
import { TcpPingTool } from "@/components/tools/TcpPingTool";
import { WhoisTool } from "@/components/tools/WhoisTool";
import { SslTool } from "@/components/tools/SslTool";
import { DebugSandbox } from "@/components/tools/DebugSandbox";

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

function SpeedTestCard() {
  const [isTesting, setIsTesting] = useState(false);
  const [result, setResult] = useState<SpeedTestPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [liveDownload, setLiveDownload] = useState(0);
  const unlistenRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    let mounted = true;
    listen<SpeedTestProgressPayload>("speed-test-progress", (e) => {
      if (!mounted) return;
      setLiveDownload(e.payload.downloadMbps ?? 0);
    }).then((unlisten) => {
      unlistenRef.current = unlisten;
    });
    return () => {
      mounted = false;
      unlistenRef.current?.();
    };
  }, []);

  const runTest = async () => {
    setIsTesting(true);
    setError(null);
    setResult(null);
    setLiveDownload(0);
    try {
      const r = await invoke<SpeedTestPayload>("run_speed_test");
      setResult(r);
    } catch (e) {
      console.error("[FLIGHT_RECORDER] [tools] speed test failed", e);
      setError(typeof e === "string" ? e : String(e));
    } finally {
      setIsTesting(false);
      setLiveDownload(0);
    }
  };

  return (
    <div className="bg-surface rounded-xl overflow-hidden">
      <div className="px-4 py-4">
        <p className="text-[15px] font-semibold text-primary">Speed Test</p>
        <p className="text-[13px] text-secondary mt-0.5">
          Ping · Download · Upload via Cloudflare
        </p>
      </div>
      {(result || (isTesting && liveDownload > 0) || error) && (
        <>
          <div className="h-px bg-separator" />
          <div className="px-4 py-4 space-y-3">
            {result && (
              <div className="grid grid-cols-3 gap-2">
                <div>
                  <p className="text-[11px] text-secondary uppercase tracking-wider">
                    Ping
                  </p>
                  <p className="text-[15px] font-medium text-primary tabular-nums">
                    {result.pingMs < 0.1
                      ? "—"
                      : `${Math.round(result.pingMs)} ms`}
                  </p>
                </div>
                <div>
                  <p className="text-[11px] text-secondary uppercase tracking-wider">
                    Download
                  </p>
                  <p className="text-[15px] font-medium text-online tabular-nums">
                    {result.downloadMbps.toFixed(1)} Mbps
                  </p>
                </div>
                <div>
                  <p className="text-[11px] text-secondary uppercase tracking-wider">
                    Upload
                  </p>
                  <p className="text-[15px] font-medium text-accent tabular-nums">
                    {result.uploadMbps.toFixed(1)} Mbps
                  </p>
                </div>
              </div>
            )}
            {isTesting && liveDownload > 0 && (
              <div>
                <p className="text-[11px] text-secondary uppercase tracking-wider">
                  Downloading…
                </p>
                <p className="text-[15px] font-medium text-online tabular-nums">
                  {liveDownload.toFixed(1)} Mbps
                </p>
              </div>
            )}
            {error && <p className="text-[13px] text-error">{error}</p>}
          </div>
        </>
      )}
      <div className="h-px bg-separator" />
      <div className="px-4 py-4">
        <button
          onClick={() => void runTest()}
          disabled={isTesting}
          className="w-full py-3.5 rounded-xl bg-accent text-white text-[16px] font-semibold text-center hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {isTesting ? "Running test…" : "Run Speed Test"}
        </button>
      </div>
    </div>
  );
}

type ToolId =
  | "ping"
  | "dns"
  | "mac"
  | "geo"
  | "subnet"
  | "http"
  | "tcpping"
  | "whois"
  | "ssl"
  | "wol"
  | "portscan"
  | "debug";

const TOOLS: Array<{
  id: ToolId;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  Component: React.ComponentType;
}> = [
  { id: "debug",    label: "Debug Sandbox",      icon: Terminal,   Component: DebugSandbox },
  { id: "ping",     label: "Ping",               icon: Activity,   Component: PingTool },
  { id: "dns",      label: "DNS Lookup",          icon: Globe,      Component: DnsLookupTool },
  { id: "mac",      label: "MAC Lookup",          icon: Cpu,        Component: MacLookupTool },
  { id: "geo",      label: "IP Geolocation",      icon: MapPin,     Component: IpGeoTool },
  { id: "subnet",   label: "Subnet Calculator",   icon: Network,    Component: SubnetCalcTool },
  { id: "http",     label: "HTTP Headers",        icon: FileCode,   Component: HttpHeaderTool },
  { id: "tcpping",  label: "TCP Ping",            icon: Zap,        Component: TcpPingTool },
  { id: "whois",    label: "WHOIS",               icon: BookOpen,   Component: WhoisTool },
  { id: "ssl",      label: "SSL Lookup",          icon: Lock,       Component: SslTool },
  { id: "wol",      label: "Wake-on-LAN",         icon: Power,      Component: WolTool },
  { id: "portscan", label: "Deep Port Scan",      icon: ScanSearch, Component: PortScannerTool },
];

export function ToolsPage() {
  const [active, setActive] = useState<ToolId | null>(null);

  const activeTool = active ? TOOLS.find((t) => t.id === active) : null;

  if (activeTool) {
    const { Component } = activeTool;
    return (
      <div className="pb-32">
        <button
          onClick={() => setActive(null)}
          className="flex items-center gap-1 mb-4 text-accent text-[15px]"
        >
          <ChevronLeft className="w-5 h-5" />
          Tools
        </button>
        <Component />
      </div>
    );
  }

  return (
    <div className="space-y-6 pb-32">
      <SpeedTestCard />

      <div>
        <p className="text-[13px] font-semibold text-secondary uppercase tracking-wider px-1 mb-3">
          Diagnostics
        </p>
        <div className="grid grid-cols-2 gap-3">
          {TOOLS.map((tool) => {
            const Icon = tool.icon;
            return (
              <button
                key={tool.id}
                onClick={() => setActive(tool.id)}
                className="bg-surface rounded-xl p-4 flex flex-col items-center gap-3 active:bg-surface-hover transition-colors"
              >
                <Icon className="w-6 h-6 text-secondary" />
                <span className="text-[13px] font-medium text-primary text-center leading-tight">
                  {tool.label}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

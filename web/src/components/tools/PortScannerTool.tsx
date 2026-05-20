import { useState } from "react";
import { invoke } from "@/lib/transport";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Loader2, ShieldCheck } from "lucide-react";

interface PortScanResult {
  openPorts: number[];
}

const SCANNED_PORTS: { port: number; label: string }[] = [
  { port: 22, label: "SSH" },
  { port: 23, label: "Telnet" },
  { port: 80, label: "HTTP" },
  { port: 443, label: "HTTPS" },
  { port: 445, label: "SMB" },
  { port: 3389, label: "RDP" },
  { port: 5000, label: "Alt-HTTP" },
  { port: 8080, label: "Alt-HTTP" },
  { port: 8443, label: "Alt-HTTPS" },
];

export function PortScannerTool() {
  const [target, setTarget] = useState("");
  const [result, setResult] = useState<PortScanResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleScan() {
    const trimmed = target.trim();
    if (!trimmed) return;

    setLoading(true);
    setResult(null);
    setError(null);

    try {
      const data = await invoke<PortScanResult>("scan_device_ports", {
        ip: trimmed,
      });
      setResult(data);
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && target.trim()) {
      void handleScan();
    }
  }

  const openSet = new Set(result?.openPorts ?? []);
  const openCount = openSet.size;

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <ShieldCheck className="size-4 text-cyan-400" />
          Port Scanner
        </CardTitle>
        <CardDescription className="text-slate-400">
          TCP connect probe across 9 high-value ports (SSH, HTTP, SMB, RDP…).
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="192.168.1.1"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handleScan()}
            disabled={loading || !target.trim()}
            className="shrink-0 bg-cyan-600 text-white hover:bg-cyan-500 disabled:opacity-50"
            size="default"
          >
            {loading ? (
              <>
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                Scanning…
              </>
            ) : (
              "Scan Ports"
            )}
          </Button>
        </div>

        {error !== null && (
          <div className="rounded-lg border border-red-800/60 bg-red-950/30 px-3 py-2 text-xs text-red-300">
            {error}
          </div>
        )}

        {result !== null && (
          <div className="space-y-3">
            <p className="text-xs text-slate-400">
              {openCount === 0
                ? "No open ports found."
                : `${openCount} open port${openCount !== 1 ? "s" : ""} found.`}
            </p>

            <div className="grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-5">
              {SCANNED_PORTS.map(({ port, label }) => {
                const isOpen = openSet.has(port);
                return (
                  <div
                    key={port}
                    className={`flex flex-col items-center gap-0.5 rounded-lg border px-2 py-2 text-center transition-colors ${
                      isOpen
                        ? "border-emerald-600/60 bg-emerald-950/40 text-emerald-300"
                        : "border-slate-700/60 bg-slate-800/40 text-slate-500"
                    }`}
                  >
                    <span className="text-[11px] font-semibold tabular-nums">
                      {port}
                    </span>
                    <span className="text-[10px] leading-tight">{label}</span>
                    <span
                      className={`mt-0.5 rounded-full px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wide ${
                        isOpen
                          ? "bg-emerald-500/20 text-emerald-400"
                          : "bg-slate-700/40 text-slate-600"
                      }`}
                    >
                      {isOpen ? "open" : "closed"}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

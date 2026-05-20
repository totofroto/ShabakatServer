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
import { Loader2, Timer } from "lucide-react";

export function TcpPingTool() {
  const [target, setTarget] = useState("");
  const [port, setPort] = useState("80");
  const [latency, setLatency] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handlePing() {
    const trimmedTarget = target.trim();
    const parsedPort = parseInt(port, 10);
    if (!trimmedTarget || isNaN(parsedPort)) return;

    setLoading(true);
    setLatency(null);
    setError(null);

    try {
      const ms = await invoke<number>("tcp_ping", {
        target: trimmedTarget,
        port: parsedPort,
      });
      setLatency(ms);
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && target.trim()) {
      void handlePing();
    }
  }

  function latencyColor(ms: number) {
    if (ms < 50) return "text-emerald-400";
    if (ms < 150) return "text-yellow-400";
    return "text-red-400";
  }

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <Timer className="size-4 text-orange-400" />
          TCP Ping
        </CardTitle>
        <CardDescription className="text-slate-400">
          Measure TCP connection latency to any host and port.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="192.168.1.1 or example.com"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-orange-500/50 disabled:opacity-50"
          />
          <input
            type="number"
            value={port}
            onChange={(e) => setPort(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="80"
            min={1}
            max={65535}
            disabled={loading}
            className="h-8 w-20 shrink-0 rounded-lg border border-slate-700 bg-slate-950 px-3 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-orange-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handlePing()}
            disabled={loading || !target.trim()}
            className="shrink-0 bg-orange-600 text-white hover:bg-orange-500 disabled:opacity-50"
            size="default"
          >
            {loading ? (
              <>
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                Pinging…
              </>
            ) : (
              "TCP Ping"
            )}
          </Button>
        </div>

        {error !== null && (
          <div className="rounded-lg border border-red-800/60 bg-red-950/30 px-3 py-2 text-xs text-red-300">
            {error}
          </div>
        )}

        {latency !== null && (
          <div className="flex items-center gap-3 rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3">
            <span className="size-2 shrink-0 rounded-full bg-orange-400" />
            <div>
              <p className="text-[10px] uppercase tracking-widest text-slate-500">
                Latency
              </p>
              <p className={`mt-0.5 font-mono text-2xl font-semibold ${latencyColor(latency)}`}>
                {latency} <span className="text-sm font-normal text-slate-400">ms</span>
              </p>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

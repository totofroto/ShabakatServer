import { useState } from "react";
import { transport } from "@/lib/transport";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Loader2, Terminal } from "lucide-react";

export function DebugSandbox() {
  const [targetIp, setTargetIp] = useState("");
  const [probeType, setProbeType] = useState("ping");
  const [result, setResult] = useState<{ online: boolean; raw_output: string } | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleProbe() {
    const trimmed = targetIp.trim();
    if (!trimmed) return;

    setLoading(true);
    setResult(null);

    try {
      const res = await transport.fetch("/api/debug/probe", {
        method: "POST",
        body: JSON.stringify({
          target_ip: trimmed,
          probe_type: probeType,
        }),
      });
      
      if (!res.ok) {
        throw new Error(`Server error (${res.status}): ${res.statusText}`);
      }
      
      const data = await res.json();
      setResult(data);
    } catch (err) {
      setResult({ 
        online: false, 
        raw_output: err instanceof Error ? err.message : String(err) 
      });
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && targetIp.trim()) {
      void handleProbe();
    }
  }

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="text-slate-100 flex items-center gap-2">
          <Terminal className="size-5 text-emerald-500" />
          Interactive Network Sandbox
        </CardTitle>
        <CardDescription className="text-slate-400">
          Granular, single-host network probing for instant debugging.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="space-y-2">
            <label className="text-xs font-bold uppercase tracking-widest text-slate-500 px-1">
              Target IP Address
            </label>
            <input
              type="text"
              value={targetIp}
              onChange={(e) => setTargetIp(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="192.168.1.X"
              disabled={loading}
              className="h-10 w-full rounded-lg border border-slate-700 bg-slate-950 px-3 text-sm text-slate-100 placeholder:text-slate-600 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 disabled:opacity-50"
            />
          </div>
          <div className="space-y-2">
            <label className="text-xs font-bold uppercase tracking-widest text-slate-500 px-1">
              Probe Type
            </label>
            <select
              value={probeType}
              onChange={(e) => setProbeType(e.target.value)}
              disabled={loading}
              className="h-10 w-full rounded-lg border border-slate-700 bg-slate-950 px-3 text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 disabled:opacity-50 appearance-none"
            >
              <option value="ping">Direct Ping (ICMP/TCP)</option>
              <option value="arp">ARP Table Check</option>
              <option value="udp_trick">UDP Trick (Kernel Force)</option>
            </select>
          </div>
        </div>

        <Button
          onClick={() => void handleProbe()}
          disabled={loading || !targetIp.trim()}
          className="w-full bg-emerald-600 text-white hover:bg-emerald-500 disabled:opacity-50 h-10 font-bold uppercase tracking-widest text-xs"
        >
          {loading ? (
            <>
              <Loader2 className="mr-2 size-4 animate-spin" />
              Executing Probe…
            </>
          ) : (
            "Execute Probe"
          )}
        </Button>

        {result && (
          <div className="space-y-2 mt-4 animate-in fade-in slide-in-from-top-2 duration-300">
             <div className="flex items-center justify-between px-1">
                <span className="text-[10px] font-black uppercase tracking-widest text-slate-500">
                    Response Output
                </span>
                <span className={`text-[10px] font-black uppercase tracking-widest ${result.online ? 'text-emerald-500' : 'text-rose-500'}`}>
                    Status: {result.online ? 'Online / Responding' : 'No Response / Offline'}
                </span>
            </div>
            <pre className="min-h-[8rem] overflow-x-auto rounded-lg border border-slate-800 bg-zinc-950 p-4 font-mono text-xs leading-relaxed text-emerald-400 shadow-inner">
              {result.raw_output || "No output returned."}
            </pre>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

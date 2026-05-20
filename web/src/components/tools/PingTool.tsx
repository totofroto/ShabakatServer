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
import { Loader2 } from "lucide-react";

export function PingTool() {
  const [target, setTarget] = useState("");
  const [output, setOutput] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handlePing() {
    const trimmed = target.trim();
    if (!trimmed) return;

    setLoading(true);
    setOutput(null);
    setError(null);

    try {
      const result = await invoke<string>("ping_device", { target: trimmed });
      setOutput(result);
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

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="text-slate-100">Ping</CardTitle>
        <CardDescription className="text-slate-400">
          Send 4 ICMP packets to any IP address or hostname.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="192.168.1.1 or google.com"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handlePing()}
            disabled={loading || !target.trim()}
            className="shrink-0 bg-cyan-600 text-white hover:bg-cyan-500 disabled:opacity-50"
            size="default"
          >
            {loading ? (
              <>
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                Pinging…
              </>
            ) : (
              "Ping"
            )}
          </Button>
        </div>

        {(output !== null || error !== null) && (
          <pre
            className={`min-h-[6rem] overflow-x-auto whitespace-pre-wrap break-all rounded-lg border p-3 font-mono text-xs leading-relaxed ${
              error !== null
                ? "border-red-800/60 bg-red-950/30 text-red-300"
                : "border-slate-700 bg-slate-950 text-green-300"
            }`}
          >
            {error !== null ? error : output}
          </pre>
        )}
      </CardContent>
    </Card>
  );
}

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
import { Loader2, Globe } from "lucide-react";

export function DnsLookupTool() {
  const [target, setTarget] = useState("");
  const [results, setResults] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleLookup() {
    const trimmed = target.trim();
    if (!trimmed) return;

    setLoading(true);
    setResults(null);
    setError(null);

    try {
      const data = await invoke<string[]>("dns_lookup", { target: trimmed });
      setResults(data);
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && target.trim()) {
      void handleLookup();
    }
  }

  const isReverse = /^[\d.:]+$/.test(target.trim());

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <Globe className="size-4 text-cyan-400" />
          DNS Lookup
        </CardTitle>
        <CardDescription className="text-slate-400">
          Forward lookup (hostname → IPs) or reverse lookup (IP → hostname).
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="example.com or 192.168.1.1"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handleLookup()}
            disabled={loading || !target.trim()}
            className="shrink-0 bg-cyan-600 text-white hover:bg-cyan-500 disabled:opacity-50"
            size="default"
          >
            {loading ? (
              <>
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                Looking up…
              </>
            ) : (
              "Lookup"
            )}
          </Button>
        </div>

        {error !== null && (
          <div className="rounded-lg border border-red-800/60 bg-red-950/30 px-3 py-2 text-xs text-red-300">
            {error}
          </div>
        )}

        {results !== null && (
          <div className="space-y-2">
            <p className="text-xs text-slate-400">
              {isReverse ? "Reverse lookup" : "Forward lookup"} —{" "}
              {results.length} result{results.length !== 1 ? "s" : ""}
            </p>
            <ul className="space-y-1.5">
              {results.map((r, i) => (
                <li
                  key={i}
                  className="flex items-center gap-2 rounded-lg border border-slate-700/60 bg-slate-800/40 px-3 py-2"
                >
                  <span className="size-1.5 shrink-0 rounded-full bg-cyan-400" />
                  <span className="font-mono text-sm text-slate-100">{r}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

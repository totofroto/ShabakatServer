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

export function WhoisTool() {
  const [domain, setDomain] = useState("");
  const [raw, setRaw] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleLookup() {
    const trimmed = domain.trim();
    if (!trimmed) return;

    setLoading(true);
    setRaw(null);
    setError(null);

    try {
      const json = await invoke<string>("whois_lookup", { domain: trimmed });
      setRaw(JSON.stringify(JSON.parse(json), null, 2));
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && domain.trim()) {
      void handleLookup();
    }
  }

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <Globe className="size-4 text-teal-400" />
          WHOIS Lookup
        </CardTitle>
        <CardDescription className="text-slate-400">
          Retrieve registration and ownership data for any domain.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={domain}
            onChange={(e) => setDomain(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="example.com"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-teal-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handleLookup()}
            disabled={loading || !domain.trim()}
            className="shrink-0 bg-teal-600 text-white hover:bg-teal-500 disabled:opacity-50"
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

        {raw !== null && (
          <pre className="max-h-80 overflow-auto rounded-lg border border-slate-700/60 bg-slate-950 px-4 py-3 font-mono text-xs leading-relaxed text-slate-300">
            {raw}
          </pre>
        )}
      </CardContent>
    </Card>
  );
}

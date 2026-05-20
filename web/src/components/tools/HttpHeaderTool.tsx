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
import { Loader2, FileSearch } from "lucide-react";

export function HttpHeaderTool() {
  const [url, setUrl] = useState("");
  const [headers, setHeaders] = useState<[string, string][] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleAnalyze() {
    const trimmed = url.trim();
    if (!trimmed) return;

    setLoading(true);
    setHeaders(null);
    setError(null);

    try {
      const result = await invoke<[string, string][]>("analyze_headers", {
        url: trimmed,
      });
      setHeaders(result);
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && url.trim()) {
      void handleAnalyze();
    }
  }

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <FileSearch className="size-4 text-sky-400" />
          HTTP Header Analyzer
        </CardTitle>
        <CardDescription className="text-slate-400">
          Inspect the response headers returned by any URL.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="https://example.com"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-sky-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handleAnalyze()}
            disabled={loading || !url.trim()}
            className="shrink-0 bg-sky-600 text-white hover:bg-sky-500 disabled:opacity-50"
            size="default"
          >
            {loading ? (
              <>
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                Fetching…
              </>
            ) : (
              "Analyze"
            )}
          </Button>
        </div>

        {error !== null && (
          <div className="rounded-lg border border-red-800/60 bg-red-950/30 px-3 py-2 text-xs text-red-300">
            {error}
          </div>
        )}

        {headers !== null && (
          <div className="overflow-hidden rounded-lg border border-slate-700/60">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-700/60 bg-slate-800/60">
                  <th className="px-3 py-2 text-left text-[10px] font-semibold uppercase tracking-widest text-slate-500">
                    Header
                  </th>
                  <th className="px-3 py-2 text-left text-[10px] font-semibold uppercase tracking-widest text-slate-500">
                    Value
                  </th>
                </tr>
              </thead>
              <tbody>
                {headers.map(([key, value], i) => (
                  <tr
                    key={i}
                    className="border-b border-slate-800/60 last:border-0 odd:bg-slate-900/40 even:bg-slate-800/20"
                  >
                    <td className="px-3 py-2 font-mono text-xs text-sky-300">
                      {key}
                    </td>
                    <td className="break-all px-3 py-2 font-mono text-xs text-slate-300">
                      {value}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

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

interface SslResult {
  status?: string;
  certificate?: {
    subject?: { common_name?: string };
    issuer?: { common_name?: string };
    valid_from?: string;
    valid_to?: string;
  };
}

function formatDate(iso?: string) {
  if (!iso) return "—";
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function isExpired(iso?: string) {
  if (!iso) return false;
  return new Date(iso) < new Date();
}

export function SslTool() {
  const [domain, setDomain] = useState("");
  const [result, setResult] = useState<SslResult | null>(null);
  const [raw, setRaw] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleInspect() {
    const trimmed = domain.trim();
    if (!trimmed) return;

    setLoading(true);
    setResult(null);
    setRaw(null);
    setError(null);

    try {
      const json = await invoke<string>("ssl_lookup", { domain: trimmed });
      try {
        const parsed: SslResult = JSON.parse(json);
        if (parsed.certificate) {
          setResult(parsed);
        } else {
          setRaw(JSON.stringify(parsed, null, 2));
        }
      } catch {
        setRaw(json);
      }
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && domain.trim()) {
      void handleInspect();
    }
  }

  const cert = result?.certificate;
  const expired = isExpired(cert?.valid_to);

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <ShieldCheck className="size-4 text-lime-400" />
          SSL/TLS Certificate Inspector
        </CardTitle>
        <CardDescription className="text-slate-400">
          Inspect the TLS certificate details for any domain.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={domain}
            onChange={(e) => setDomain(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="google.com"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-lime-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handleInspect()}
            disabled={loading || !domain.trim()}
            className="shrink-0 bg-lime-600 text-white hover:bg-lime-500 disabled:opacity-50"
            size="default"
          >
            {loading ? (
              <>
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                Inspecting…
              </>
            ) : (
              "Inspect"
            )}
          </Button>
        </div>

        {error !== null && (
          <div className="rounded-lg border border-red-800/60 bg-red-950/30 px-3 py-2 text-xs text-red-300">
            {error}
          </div>
        )}

        {cert && (
          <div className="space-y-2 rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3">
            <div className="grid grid-cols-2 gap-x-4 gap-y-3">
              <div>
                <p className="text-[10px] uppercase tracking-widest text-slate-500">Subject</p>
                <p className="mt-0.5 font-mono text-sm font-medium text-slate-100">
                  {cert.subject?.common_name ?? "—"}
                </p>
              </div>
              <div>
                <p className="text-[10px] uppercase tracking-widest text-slate-500">Issued By</p>
                <p className="mt-0.5 font-mono text-sm font-medium text-slate-100">
                  {cert.issuer?.common_name ?? "—"}
                </p>
              </div>
              <div>
                <p className="text-[10px] uppercase tracking-widest text-slate-500">Valid From</p>
                <p className="mt-0.5 font-mono text-sm font-medium text-slate-100">
                  {formatDate(cert.valid_from)}
                </p>
              </div>
              <div>
                <p className="text-[10px] uppercase tracking-widest text-slate-500">Valid To</p>
                <p className={`mt-0.5 font-mono text-sm font-medium ${expired ? "text-red-400" : "text-lime-400"}`}>
                  {formatDate(cert.valid_to)}
                  {expired && <span className="ml-2 text-[10px] uppercase tracking-widest">Expired</span>}
                </p>
              </div>
            </div>
          </div>
        )}

        {raw !== null && (
          <pre className="max-h-72 overflow-auto rounded-lg border border-slate-700/60 bg-slate-950 px-4 py-3 font-mono text-xs leading-relaxed text-slate-300">
            {raw}
          </pre>
        )}
      </CardContent>
    </Card>
  );
}

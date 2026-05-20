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
import { Loader2, Cpu } from "lucide-react";

export function MacLookupTool() {
  const [mac, setMac] = useState("");
  const [vendor, setVendor] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleLookup() {
    const trimmed = mac.trim();
    if (!trimmed) return;

    setLoading(true);
    setVendor(null);
    setError(null);

    try {
      const result = await invoke<string>("mac_lookup", { mac: trimmed });
      setVendor(result);
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && mac.trim()) {
      void handleLookup();
    }
  }

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <Cpu className="size-4 text-cyan-400" />
          MAC Lookup
        </CardTitle>
        <CardDescription className="text-slate-400">
          Identify the hardware vendor from a MAC address OUI prefix.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={mac}
            onChange={(e) => setMac(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="00:11:22:33:44:55"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-cyan-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handleLookup()}
            disabled={loading || !mac.trim()}
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

        {vendor !== null && (
          <div className="flex items-center gap-3 rounded-lg border border-slate-700/60 bg-slate-800/40 px-3 py-3">
            <span className="size-2 shrink-0 rounded-full bg-cyan-400" />
            <div>
              <p className="text-[10px] uppercase tracking-widest text-slate-500">
                Vendor
              </p>
              <p className="mt-0.5 text-sm font-medium text-slate-100">
                {vendor}
              </p>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

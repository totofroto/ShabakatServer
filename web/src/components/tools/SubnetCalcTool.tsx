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
import { Loader2, Network } from "lucide-react";

interface SubnetResult {
  network: string;
  broadcast: string;
  mask: string;
  prefix: number;
  hosts: number;
}

const FIELDS: { key: keyof SubnetResult; label: string }[] = [
  { key: "network", label: "Network Address" },
  { key: "broadcast", label: "Broadcast Address" },
  { key: "mask", label: "Subnet Mask" },
  { key: "prefix", label: "Prefix Length" },
  { key: "hosts", label: "Usable Hosts" },
];

export function SubnetCalcTool() {
  const [cidr, setCidr] = useState("");
  const [result, setResult] = useState<SubnetResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleCalc() {
    const trimmed = cidr.trim();
    if (!trimmed) return;

    setLoading(true);
    setResult(null);
    setError(null);

    try {
      const data = await invoke<SubnetResult>("subnet_calc", { cidr: trimmed });
      setResult(data);
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading && cidr.trim()) {
      void handleCalc();
    }
  }

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <Network className="size-4 text-violet-400" />
          Subnet Calculator
        </CardTitle>
        <CardDescription className="text-slate-400">
          Break down any CIDR block into its network details.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={cidr}
            onChange={(e) => setCidr(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="192.168.1.0/24"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-violet-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handleCalc()}
            disabled={loading || !cidr.trim()}
            className="shrink-0 bg-violet-600 text-white hover:bg-violet-500 disabled:opacity-50"
            size="default"
          >
            {loading ? (
              <>
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                Calculating…
              </>
            ) : (
              "Calculate"
            )}
          </Button>
        </div>

        {error !== null && (
          <div className="rounded-lg border border-red-800/60 bg-red-950/30 px-3 py-2 text-xs text-red-300">
            {error}
          </div>
        )}

        {result !== null && (
          <div className="grid grid-cols-2 gap-x-4 gap-y-3 rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3">
            {FIELDS.map(({ key, label }) => (
              <div key={key}>
                <p className="text-[10px] uppercase tracking-widest text-slate-500">
                  {label}
                </p>
                <p className="mt-0.5 font-mono text-sm font-medium text-slate-100">
                  {key === "prefix" ? `/${result[key]}` : String(result[key])}
                </p>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

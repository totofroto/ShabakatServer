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
import { Loader2, MapPin } from "lucide-react";

interface GeoResult {
  status: string;
  country?: string;
  countryCode?: string;
  regionName?: string;
  city?: string;
  zip?: string;
  lat?: number;
  lon?: number;
  timezone?: string;
  isp?: string;
  org?: string;
  query?: string;
  message?: string;
}

const DISPLAY_FIELDS: { key: keyof GeoResult; label: string }[] = [
  { key: "query", label: "IP Address" },
  { key: "country", label: "Country" },
  { key: "regionName", label: "Region" },
  { key: "city", label: "City" },
  { key: "zip", label: "Postal Code" },
  { key: "timezone", label: "Timezone" },
  { key: "isp", label: "ISP" },
  { key: "org", label: "Organization" },
];

export function IpGeoTool() {
  const [ip, setIp] = useState("");
  const [geo, setGeo] = useState<GeoResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleLocate() {
    setLoading(true);
    setGeo(null);
    setError(null);

    try {
      const raw = await invoke<string>("ip_geolocation", {
        ip: ip.trim() || null,
      });
      const parsed: GeoResult = JSON.parse(raw);
      if (parsed.status === "fail") {
        setError(parsed.message ?? "Lookup failed");
      } else {
        setGeo(parsed);
      }
    } catch (err) {
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !loading) {
      void handleLocate();
    }
  }

  return (
    <Card className="border-slate-800/90 bg-slate-900/80">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-slate-100">
          <MapPin className="size-4 text-emerald-400" />
          IP Geolocation
        </CardTitle>
        <CardDescription className="text-slate-400">
          Look up geographic and ISP information for any IP address.
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex gap-2">
          <input
            type="text"
            value={ip}
            onChange={(e) => setIp(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Leave blank for your public IP"
            disabled={loading}
            className="h-8 min-w-0 flex-1 rounded-lg border border-slate-700 bg-slate-950 px-3 font-mono text-sm text-slate-100 placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-emerald-500/50 disabled:opacity-50"
          />
          <Button
            onClick={() => void handleLocate()}
            disabled={loading}
            className="shrink-0 bg-emerald-600 text-white hover:bg-emerald-500 disabled:opacity-50"
            size="default"
          >
            {loading ? (
              <>
                <Loader2 className="mr-1.5 size-3.5 animate-spin" />
                Locating…
              </>
            ) : (
              "Locate"
            )}
          </Button>
        </div>

        {error !== null && (
          <div className="rounded-lg border border-red-800/60 bg-red-950/30 px-3 py-2 text-xs text-red-300">
            {error}
          </div>
        )}

        {geo !== null && (
          <div className="grid grid-cols-2 gap-x-4 gap-y-3 rounded-lg border border-slate-700/60 bg-slate-800/40 px-4 py-3">
            {DISPLAY_FIELDS.map(({ key, label }) =>
              geo[key] != null ? (
                <div key={key}>
                  <p className="text-[10px] uppercase tracking-widest text-slate-500">
                    {label}
                  </p>
                  <p className="mt-0.5 text-sm font-medium text-slate-100">
                    {String(geo[key])}
                  </p>
                </div>
              ) : null,
            )}
            {geo.lat != null && geo.lon != null && (
              <div className="col-span-2">
                <p className="text-[10px] uppercase tracking-widest text-slate-500">
                  Coordinates
                </p>
                <p className="mt-0.5 font-mono text-sm text-slate-100">
                  {geo.lat}, {geo.lon}
                </p>
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

import { useEffect, useState } from "react";
import { invoke } from "@/lib/transport";
import { Activity, BellRing } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

type HistoryEntry = {
  scanId: string;
  scannedAt: number;
  ip: string;
  isOnline: boolean;
  latencyMs: number | null;
  mac: string;
};

type DeviceEvent = {
  id: number;
  eventType: string;
  timestamp: number;
  details: string | null;
  mac: string | null;
};

type TimelineEntry = {
  id: string;
  timestamp: number;
  kind: "scan" | "new_device";
  label: string;
  detail: string;
};

export function ActivityPage() {
  const [timeline, setTimeline] = useState<TimelineEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    Promise.all([
      invoke<HistoryEntry[]>("get_history", { limit: 300 }),
      invoke<DeviceEvent[]>("get_events"),
    ])
      .then(([history, events]) => {
        if (cancelled) return;
        const entries: TimelineEntry[] = [];

        // Safety: if the transport returned an error object instead of an array,
        // treat it as empty to avoid "is not iterable" crashes.
        const safeHistory = Array.isArray(history) ? history : [];
        const safeEvents = Array.isArray(events) ? events : [];

        // Group scan history by scanId — one timeline entry per scan
        const scanMap = new Map<string, { ts: number; count: number }>();
        for (const h of safeHistory) {
          const s = scanMap.get(h.scanId);
          if (!s) {
            scanMap.set(h.scanId, { ts: h.scannedAt, count: 1 });
          } else {
            if (h.scannedAt > s.ts) s.ts = h.scannedAt;
            s.count++;
          }
        }
        for (const [scanId, { ts, count }] of scanMap) {
          entries.push({
            id: `scan-${scanId}`,
            timestamp: ts,
            kind: "scan",
            label: `Scan completed — ${count} device${count !== 1 ? "s" : ""} online`,
            detail: scanId,
          });
        }

        // New-device events
        for (const ev of safeEvents) {
          if (ev.eventType !== "new_device") continue;
          let ip = "";
          let vendor = "";
          try {
            if (ev.details) {
              const d = JSON.parse(ev.details) as {
                ip?: string;
                vendor?: string;
              };
              ip = d.ip ?? "";
              vendor =
                d.vendor && d.vendor !== "Unknown" ? ` · ${d.vendor}` : "";
            }
          } catch {
            /* ignore */
          }
          entries.push({
            id: `event-${ev.id}`,
            timestamp: ev.timestamp,
            kind: "new_device",
            label: `New device detected${vendor}`,
            detail: ip || ev.mac || "",
          });
        }

        entries.sort((a, b) => b.timestamp - a.timestamp);
        setTimeline(entries);
        setError(null);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(
          err instanceof Error ? err.message : "Failed to load activity",
        );
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="space-y-6">
      <header>
        <h2 className="text-3xl font-semibold text-primary">Activity</h2>
        <p className="mt-1 text-sm text-secondary">
          Network scans and device events logged by the server.
        </p>
      </header>

      <Card>
        <CardHeader>
          <CardTitle className="text-primary">Event Timeline</CardTitle>
          <CardDescription className="text-secondary">
            Completed scans and new-device alerts from the background monitor.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {loading ? (
            <p className="text-sm text-secondary">Loading activity…</p>
          ) : error ? (
            <p className="text-sm text-error">{error}</p>
          ) : timeline.length === 0 ? (
            <div className="rounded-md border border-separator bg-surface p-4 text-sm text-secondary">
              No activity recorded yet. Run a network scan to see events here.
            </div>
          ) : (
            <div className="max-h-[32rem] space-y-2 overflow-y-auto pr-1">
              {timeline.map((entry) => (
                <div
                  key={entry.id}
                  className="flex items-start gap-3 rounded-md bg-surface-alt p-3 text-sm"
                >
                  {entry.kind === "scan" ? (
                    <Activity
                      className="mt-0.5 size-4 shrink-0 text-accent"
                      aria-hidden
                    />
                  ) : (
                    <BellRing
                      className="mt-0.5 size-4 shrink-0 text-warning"
                      aria-hidden
                    />
                  )}
                  <div className="min-w-0">
                    <p className="font-medium text-primary">{entry.label}</p>
                    {entry.detail ? (
                      <p className="truncate font-mono text-xs text-secondary">
                        {entry.detail}
                      </p>
                    ) : null}
                    <p className="text-xs text-tertiary">
                      {new Date(entry.timestamp).toLocaleString()}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

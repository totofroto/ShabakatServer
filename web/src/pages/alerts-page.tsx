import { invoke } from "@/lib/transport";
import { listen } from "@/lib/transport";
import { BellRing, ShieldAlert } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

type NewDeviceEvent = {
  timestampMs: number;
  mac: string;
  name: string;
  ip: string;
};

export function AlertsPage() {
  const [events, setEvents] = useState<NewDeviceEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [acknowledgingMac, setAcknowledgingMac] = useState<string | null>(null);

  const sortedEvents = useMemo(
    () => [...events].sort((a, b) => b.timestampMs - a.timestampMs),
    [events],
  );

  const loadEvents = async () => {
    try {
      const result = await invoke<NewDeviceEvent[]>("get_new_device_events");
      setEvents(result);
      setError(null);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Failed to load alert events",
      );
    }
  };

  const acknowledgeDevice = async (mac: string) => {
    setAcknowledgingMac(mac);
    try {
      await invoke("acknowledge_device", { mac });
      await loadEvents();
    } catch (ackError) {
      setError(
        ackError instanceof Error
          ? ackError.message
          : "Failed to acknowledge device",
      );
    } finally {
      setAcknowledgingMac(null);
    }
  };

  useEffect(() => {
    loadEvents();
  }, []);

  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | null = null;

    const setup = async () => {
      unlisten = await listen<NewDeviceEvent>("new-device", (event) => {
        if (!isMounted) {
          return;
        }
        setEvents((prev) => [event.payload, ...prev]);
      });
    };

    setup().catch((listenError) => {
      if (!isMounted) {
        return;
      }
      setError(
        listenError instanceof Error
          ? listenError.message
          : "Failed to subscribe to new-device events",
      );
    });

    return () => {
      isMounted = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  return (
    <div className="space-y-6">
      <header>
        <h2 className="text-3xl font-semibold text-primary">Alerts</h2>
        <p className="mt-1 text-sm text-secondary">
          Background monitor detections from scheduled quiet scans.
        </p>
      </header>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-primary">
            <ShieldAlert className="size-5 text-warning" aria-hidden />
            New Device Alerts
          </CardTitle>
          <CardDescription className="text-secondary">
            Unrecognized MAC addresses detected by the 5-minute background
            monitor.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {error ? <p className="text-sm text-error">{error}</p> : null}

          {sortedEvents.length === 0 ? (
            <div className="rounded-md border border-separator bg-surface p-4 text-sm text-secondary">
              No new-device alerts yet.
            </div>
          ) : (
            <div className="max-h-[28rem] space-y-3 overflow-y-auto pr-1">
              {sortedEvents.map((event, index) => (
                <div
                  key={`${event.mac}-${event.timestampMs}-${index}`}
                  className="rounded-md border border-separator bg-surface p-3 text-sm text-secondary"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <p className="flex items-center gap-2 font-medium text-primary">
                        <BellRing
                          className="size-4 text-warning"
                          aria-hidden
                        />
                        New Device Joined
                      </p>
                      <p className="mt-1 text-xs text-secondary">
                        {new Date(event.timestampMs).toLocaleString()}
                      </p>
                    </div>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      disabled={acknowledgingMac === event.mac}
                      onClick={() => acknowledgeDevice(event.mac)}
                    >
                      {acknowledgingMac === event.mac
                        ? "Acknowledging..."
                        : "Acknowledge"}
                    </Button>
                  </div>
                  <p className="mt-3 text-sm text-primary">
                    {event.name} ({event.ip})
                  </p>
                  <p className="mt-1 font-mono text-xs text-secondary">
                    MAC: {event.mac}
                  </p>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { isTauri } from "@/lib/transport";
import { listen, type UnlistenFn } from "@/lib/transport";

export type NetworkChangePayload = {
  changeType: string;
  ip: string;
  mac: string;
  name: string;
};

export type NotificationCenterItem = {
  id: string;
  receivedAt: number;
  changeType: string;
  /** One-line copy for the list, e.g. "Tesla Model 3 went offline at 12:05 PM" */
  summary: string;
  ip: string;
  mac: string;
  name: string;
  read: boolean;
};

const MAX_ITEMS = 150;

function formatTimeLabel(d: Date): string {
  return d.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

export function formatNetworkChangeSummary(
  changeType: string,
  name: string,
  ip: string,
  at: Date,
): string {
  const t = formatTimeLabel(at);
  const label = name?.trim() || "Device";
  switch (changeType) {
    case "device_offline":
      return `${label} went offline at ${t}`;
    case "new_unknown_device":
      return `Unknown device at ${ip} appeared at ${t}`;
    case "new_device":
      return `${label} joined the network at ${t}`;
    default:
      return `${label} · ${changeType} at ${t}`;
  }
}

type NotificationCenterContextValue = {
  open: boolean;
  setOpen: (open: boolean) => void;
  items: NotificationCenterItem[];
  unreadCount: number;
  clearHistory: () => void;
};

const NotificationCenterContext = createContext<
  NotificationCenterContextValue | undefined
>(undefined);

export function NotificationCenterProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<NotificationCenterItem[]>([]);
  const panelOpenRef = useRef(false);
  panelOpenRef.current = open;

  useEffect(() => {
    if (!isTauri() || typeof window === "undefined") {
      return;
    }

    let unlisten: UnlistenFn | undefined;

    void (async () => {
      try {
        unlisten = await listen<NetworkChangePayload>(
          "network-change-detected",
          (event) => {
            const p = event.payload;
            const receivedAt = Date.now();
            const at = new Date(receivedAt);
            const summary = formatNetworkChangeSummary(
              p.changeType,
              p.name,
              p.ip,
              at,
            );
            const id =
              typeof crypto !== "undefined" && "randomUUID" in crypto
                ? crypto.randomUUID()
                : `${receivedAt}-${Math.random().toString(36).slice(2)}`;

            setItems((prev) => {
              const next: NotificationCenterItem[] = [
                {
                  id,
                  receivedAt,
                  changeType: p.changeType,
                  summary,
                  ip: p.ip,
                  mac: p.mac,
                  name: p.name,
                  read: panelOpenRef.current,
                },
                ...prev,
              ];
              return next.slice(0, MAX_ITEMS);
            });
          },
        );
      } catch (err) {
        console.error("NotificationCenter: listen failed", err);
      }
    })();

    return () => {
      void unlisten?.();
    };
  }, []);

  const setOpenTracked = useCallback((next: boolean) => {
    setOpen(next);
    if (next) {
      setItems((prev) => prev.map((e) => ({ ...e, read: true })));
    }
  }, []);

  const clearHistory = useCallback(() => {
    setItems([]);
  }, []);

  const unreadCount = useMemo(
    () => items.filter((e) => !e.read).length,
    [items],
  );

  const value = useMemo(
    () => ({
      open,
      setOpen: setOpenTracked,
      items,
      unreadCount,
      clearHistory,
    }),
    [clearHistory, items, open, setOpenTracked, unreadCount],
  );

  return (
    <NotificationCenterContext.Provider value={value}>
      {children}
    </NotificationCenterContext.Provider>
  );
}

export function useNotificationCenter() {
  const ctx = useContext(NotificationCenterContext);
  if (!ctx) {
    throw new Error(
      "useNotificationCenter must be used within NotificationCenterProvider",
    );
  }
  return ctx;
}

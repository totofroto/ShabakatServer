import { Bell, Trash2 } from "lucide-react";
import { isTauri } from "@/lib/transport";
import { useNotificationCenter } from "@/context/NotificationCenterContext";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { cn } from "@/lib/utils";

function changeTypeBadge(changeType: string): { label: string; className: string } {
  switch (changeType) {
    case "device_offline":
      return {
        label: "Offline",
        className: "border-error/40 bg-error/15 text-error",
      };
    case "new_unknown_device":
      return {
        label: "Unknown",
        className: "border-warning/40 bg-warning/15 text-warning",
      };
    case "new_device":
      return {
        label: "New",
        className: "border-online/40 bg-online/15 text-online",
      };
    default:
      return {
        label: changeType,
        className: "border-separator bg-surface-alt text-secondary",
      };
  }
}

const bellButtonClass =
  "relative flex size-10 items-center justify-center rounded-xl border border-separator bg-surface-alt text-secondary transition hover:bg-surface hover:text-primary";

/** Opens the notification sheet; mount once per placement (e.g. mobile header + desktop sidebar). */
export function NotificationCenterBell() {
  const { setOpen, unreadCount } = useNotificationCenter();

  if (!isTauri()) {
    return null;
  }

  return (
    <button
      type="button"
      onClick={() => setOpen(true)}
      className={bellButtonClass}
      aria-label={`Notifications${unreadCount ? `, ${unreadCount} unread` : ""}`}
    >
      <Bell className="size-5" aria-hidden />
      {unreadCount > 0 ? (
        <span className="absolute -right-0.5 -top-0.5 flex min-w-[1.1rem] items-center justify-center rounded-full bg-error px-1 text-[10px] font-bold leading-none text-white">
          {unreadCount > 99 ? "99+" : unreadCount}
        </span>
      ) : null}
    </button>
  );
}

/** Right sheet for `network-change-detected` history; mount once at app shell root. */
export function NotificationCenterPanel() {
  const { open, setOpen, items, clearHistory } = useNotificationCenter();

  if (!isTauri()) {
    return null;
  }

  return (
    <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent className="z-[110] flex max-h-[100dvh] max-w-md flex-col border-l border-separator bg-surface p-0 pt-14 animate-in fade-in slide-in-from-right duration-300">
          <SheetHeader className="shrink-0 border-b border-separator px-6 pb-4 pr-14">
            <SheetTitle className="text-primary">Notification Center</SheetTitle>
            <SheetDescription className="text-secondary">
              Live Watch network changes — hosts going offline or new devices on
              your LAN.
            </SheetDescription>
            {items.length > 0 ? (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="mt-2 h-8 w-fit gap-1.5 px-2 text-xs text-secondary hover:bg-surface-alt hover:text-primary"
                onClick={() => clearHistory()}
              >
                <Trash2 className="size-3.5" aria-hidden />
                Clear history
              </Button>
            ) : null}
          </SheetHeader>

          <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-8 pt-2">
            {items.length === 0 ? (
              <div className="rounded-xl border border-separator bg-surface-alt px-4 py-10 text-center text-sm text-secondary">
                No events yet. When Live Watch detects a change, it will appear
                here.
              </div>
            ) : (
              <ul className="space-y-2">
                {items.map((item) => {
                  const badge = changeTypeBadge(item.changeType);
                  const time = new Date(item.receivedAt).toLocaleString(undefined, {
                    month: "short",
                    day: "numeric",
                    hour: "numeric",
                    minute: "2-digit",
                  });
                  return (
                    <li key={item.id}>
                      <div
                        className={cn(
                          "rounded-xl border p-3 transition-colors",
                          item.read
                            ? "border-separator bg-surface-alt"
                            : "border-accent/25 bg-accent-muted",
                        )}
                      >
                        <div className="flex items-start justify-between gap-2">
                          <p className="text-sm font-medium leading-snug text-primary">
                            {item.summary}
                          </p>
                          <span
                            className={cn(
                              "shrink-0 rounded-md border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                              badge.className,
                            )}
                          >
                            {badge.label}
                          </span>
                        </div>
                        <p className="mt-2 font-mono text-[11px] text-tertiary">
                          {item.ip} · {item.mac}
                        </p>
                        <p className="mt-1 text-[10px] uppercase tracking-wide text-tertiary">
                          {time}
                        </p>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        </SheetContent>
      </Sheet>
  );
}

/** Bell + panel together (single placement). Prefer `NotificationCenterBell` + `NotificationCenterPanel` when the bell appears in multiple places. */
export function NotificationCenterDrawer() {
  if (!isTauri()) {
    return null;
  }

  return (
    <>
      <NotificationCenterBell />
      <NotificationCenterPanel />
    </>
  );
}

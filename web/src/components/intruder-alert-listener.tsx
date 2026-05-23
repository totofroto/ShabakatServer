import { listen } from "@/lib/transport";
import { isTauri } from "@/lib/transport";
import { vibrate } from "@tauri-apps/plugin-haptics";
import { ShieldAlert, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

export type IntruderAlertPayload = {
  timestampMs: number;
  mac: string;
  name: string;
  ip: string;
};

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

/** Triple SOS-style pulse: 200 / gap 100 / 200 / gap 100 / 500 ms (plugin API is per-duration, not pattern array). */
async function tripleSosVibrate(): Promise<void> {
  if (!isTauri()) {
    return;
  }
  try {
    await vibrate(200);
    await sleep(100);
    await vibrate(200);
    await sleep(100);
    await vibrate(500);
  } catch {
    /* Haptics unavailable on desktop / simulator */
  }
}

export function IntruderAlertListener() {
  const [toast, setToast] = useState<IntruderAlertPayload | null>(null);
  const dismissTimer = useRef<number | null>(null);

  const dismiss = useCallback(() => {
    if (dismissTimer.current !== null) {
      window.clearTimeout(dismissTimer.current);
      dismissTimer.current = null;
    }
    setToast(null);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    const setup = async () => {
      unlisten = await listen<IntruderAlertPayload>(
        "new-device",
        (event) => {
          if (cancelled) {
            return;
          }
          void tripleSosVibrate();
          setToast(event.payload);
        },
      );
    };

    void setup().catch((err) => {
      console.error("intruder-alert listener failed:", err);
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!toast) {
      return;
    }
    if (dismissTimer.current !== null) {
      window.clearTimeout(dismissTimer.current);
    }
    dismissTimer.current = window.setTimeout(() => {
      dismiss();
    }, 12_000);
    return () => {
      if (dismissTimer.current !== null) {
        window.clearTimeout(dismissTimer.current);
        dismissTimer.current = null;
      }
    };
  }, [toast, dismiss]);

  if (!toast) {
    return null;
  }

  return (
    <div
      role="alert"
      aria-live="assertive"
      className="pointer-events-auto fixed inset-x-4 top-[max(1rem,env(safe-area-inset-top))] z-[100] md:inset-x-auto md:left-1/2 md:w-full md:max-w-md md:-translate-x-1/2"
    >
      <div className="relative flex items-start gap-3 rounded-xl border border-error/30 bg-surface p-4 pr-12">
        <ShieldAlert
          className="size-8 shrink-0 text-error"
          aria-hidden
        />
        <div className="min-w-0 flex-1">
          <p className="text-xs font-bold uppercase tracking-widest text-error">
            New device
          </p>
          <p className="mt-1 font-semibold text-primary">
            {toast.name?.trim() || "Unknown device"}
          </p>
          <p className="mt-0.5 font-mono text-sm text-accent">{toast.ip}</p>
          <p className="mt-1 font-mono text-xs text-tertiary">{toast.mac}</p>
        </div>
        <button
          type="button"
          onClick={dismiss}
          className="absolute right-3 top-3 rounded-md p-1 text-secondary transition hover:bg-surface-alt hover:text-primary"
          aria-label="Dismiss intruder alert"
        >
          <X className="size-5" aria-hidden />
        </button>
      </div>
    </div>
  );
}

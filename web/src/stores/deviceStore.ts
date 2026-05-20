import { create } from "zustand";
import type { DeviceRow } from "@/hooks/useNetworkScan";

type DeviceStoreState = {
  /**
   * Non-reactive identity map — the single source of truth for device history.
   * Mutated in-place by actions; never replaced wholesale. Do NOT subscribe to
   * this field for rendering — use `devices` instead.
   */
  _map: Map<string, DeviceRow>;
  /** Sorted reactive snapshot of `_map.values()`. Components subscribe to this. */
  devices: DeviceRow[];
};

type DeviceStoreActions = {
  /**
   * Replace the reactive `devices` array. Caller is responsible for sorting.
   * Call this after every mutation to `_map` that should trigger a re-render.
   */
  setDevices: (devices: DeviceRow[]) => void;
  /**
   * Clear `_map` and reset `devices` to []. Used on network-scan-reset events.
   */
  resetStore: () => void;
  /**
   * Patch a single device by IP, updating both `_map` and `devices`.
   * Returns the updated row, or null when the device is not found.
   */
  patchDevice: (ip: string, changes: Partial<DeviceRow>) => DeviceRow | null;
};

export type DeviceStore = DeviceStoreState & DeviceStoreActions;

export const useDeviceStore = create<DeviceStore>((set, get) => ({
  _map: new Map<string, DeviceRow>(),
  devices: [],

  setDevices: (devices) => set({ devices }),

  resetStore: () => {
    get()._map.clear();
    set({ devices: [] });
  },

  patchDevice: (ip, changes) => {
    const m = get()._map;
    for (const [k, d] of m) {
      if (d.ip === ip) {
        const updated: DeviceRow = { ...d, ...changes };
        m.set(k, updated);
        set({ devices: get().devices.map((r) => (r.ip === ip ? updated : r)) });
        return updated;
      }
    }
    return null;
  },
}));

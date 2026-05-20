import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { isTauri } from "@/lib/transport";
import {
  getNetworkState,
  lanScanAllowedForState,
  onNetworkStateChanged,
  type NetworkState,
} from "@/lib/network-state";

type NetworkConnectivityValue = {
  networkState: NetworkState;
  lanScanAllowed: boolean;
  refresh: () => Promise<void>;
};

const NetworkConnectivityContext = createContext<
  NetworkConnectivityValue | undefined
>(undefined);

export function NetworkConnectivityProvider({
  children,
}: {
  children: ReactNode;
}) {
  const [networkState, setNetworkState] = useState<NetworkState>(() => ({
    isOnline: typeof navigator !== "undefined" ? navigator.onLine : true,
    connectionType: "unknown",
  }));

  const refresh = useCallback(async () => {
    setNetworkState(await getNetworkState());
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      try {
        const initial = await getNetworkState();
        if (!cancelled) {
          setNetworkState(initial);
        }
        unlisten = await onNetworkStateChanged(
          (next) => {
            if (!cancelled) {
              setNetworkState(next);
            }
          },
          { emitInitial: false },
        );
      } catch {
        if (!cancelled) {
          await refresh();
        }
      }
    };

    void setup();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refresh]);

  const lanScanAllowed = useMemo(() => {
    if (!isTauri()) {
      return true;
    }
    return lanScanAllowedForState(networkState);
  }, [networkState]);

  const value = useMemo(
    () => ({
      networkState,
      lanScanAllowed,
      refresh,
    }),
    [networkState, lanScanAllowed, refresh],
  );

  return (
    <NetworkConnectivityContext.Provider value={value}>
      {children}
    </NetworkConnectivityContext.Provider>
  );
}

export function useNetworkConnectivity(): NetworkConnectivityValue {
  const ctx = useContext(NetworkConnectivityContext);
  if (!ctx) {
    throw new Error(
      "useNetworkConnectivity must be used within NetworkConnectivityProvider",
    );
  }
  return ctx;
}

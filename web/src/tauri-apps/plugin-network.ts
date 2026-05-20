/** Network state helpers backed by the custom `get_active_ip` Rust command + browser APIs. */
export {
  type ConnectionType,
  type NetworkState,
  type NetworkStateListenerOptions,
  getNetworkState,
  lanScanAllowedForState,
  onNetworkStateChanged,
} from "@/lib/network-state";

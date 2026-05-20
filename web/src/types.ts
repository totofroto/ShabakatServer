import type { DeviceRow } from "@/hooks/useNetworkScan";

/**
 * Network device as discovered by the scanner and shown in the UI.
 * Same shape as `DeviceRow` from the scan hook / plugin store.
 */
export type Device = DeviceRow;

export interface DnsProvider {
  id: string;
  name: string;
  ip: string;
  port: number;
  username?: string;
  password?: string;
  isEnabled: boolean;
  createdAt: number;
}

import { invoke } from "@tauri-apps/api/core";

export interface HostInfo {
  appVersion: string;
  protocolVersion: number;
  securityBoundary: "rust-host" | "browser-mock";
}

export async function getHostInfo(): Promise<HostInfo> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return { appVersion: "0.1.0-dev", protocolVersion: 1, securityBoundary: "browser-mock" };
  }
  return invoke<HostInfo>("get_host_info");
}

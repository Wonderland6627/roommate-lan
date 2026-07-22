import { invoke } from "@tauri-apps/api/core";
import type { NetworkStatusDto } from "../types/network";

export async function apiConnect(opts?: {
  loginServer?: string;
  authKey?: string;
}): Promise<string> {
  return invoke<string>("connect", {
    loginServer: opts?.loginServer ?? null,
    authKey: opts?.authKey ?? null,
  });
}

export async function apiDisconnect(): Promise<string> {
  return invoke<string>("disconnect");
}

export async function apiGetStatus(): Promise<NetworkStatusDto> {
  return invoke<NetworkStatusDto>("get_status");
}

export async function apiPingPeer(ip: string): Promise<number> {
  return invoke<number>("ping_peer", { ip });
}

export async function apiIsAdmin(): Promise<boolean> {
  return invoke<boolean>("is_admin");
}

export async function apiSidecarVersion(): Promise<string> {
  return invoke<string>("sidecar_version");
}

export async function apiBootstrapUrl(): Promise<string> {
  return invoke<string>("bootstrap_url");
}

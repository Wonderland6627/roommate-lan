import { listen } from "@tauri-apps/api/event";
import { onMounted, onUnmounted, ref, shallowRef } from "vue";
import {
  apiConnect,
  apiDisconnect,
  apiGetStatus,
  apiIsAdmin,
  apiPingPeer,
} from "../lib/tauri";
import type {
  ConnectionPhase,
  NetworkStatusDto,
  PeerView,
} from "../types/network";

const STATUS_MS = 2000;
const PING_MS = 5000;

export function useNetworkStatus() {
  const phase = ref<ConnectionPhase>("idle");
  const error = ref<string | null>(null);
  const status = shallowRef<NetworkStatusDto | null>(null);
  const isAdmin = ref(true);
  const latencies = ref<Record<string, number>>({});

  let statusTimer: ReturnType<typeof setInterval> | null = null;
  let pingTimer: ReturnType<typeof setInterval> | null = null;
  let unlistenAuto: (() => void) | null = null;

  async function refreshAdmin() {
    try {
      isAdmin.value = await apiIsAdmin();
    } catch {
      isAdmin.value = true;
    }
  }

  function stopPolling() {
    if (statusTimer) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
    if (pingTimer) {
      clearInterval(pingTimer);
      pingTimer = null;
    }
  }

  async function pullStatus() {
    try {
      const next = await apiGetStatus();
      status.value = next;
      error.value = null;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // While connecting, status may fail until daemon is up.
      if (phase.value === "connected") {
        error.value = msg;
      }
    }
  }

  async function pullPings() {
    const peers = status.value?.peers ?? [];
    const online = peers.filter((p) => p.online && p.ips[0]);
    await Promise.all(
      online.map(async (peer) => {
        const ip = peer.ips[0];
        try {
          const ms = await apiPingPeer(ip);
          latencies.value = { ...latencies.value, [peer.id]: ms };
        } catch {
          // ignore single-peer ping failures
        }
      }),
    );
  }

  function startPolling() {
    stopPolling();
    void pullStatus();
    statusTimer = setInterval(() => void pullStatus(), STATUS_MS);
    pingTimer = setInterval(() => void pullPings(), PING_MS);
  }

  async function connect() {
    error.value = null;
    phase.value = "connecting";
    try {
      await apiConnect();
      phase.value = "connected";
      startPolling();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("管理员") || msg.includes("UAC")) {
        phase.value = "idle";
        error.value = "已请求管理员权限，确认 UAC 后窗口会自动重开并继续连接…";
        return;
      }
      phase.value = "error";
      error.value = msg;
    }
  }

  async function disconnect() {
    phase.value = "disconnecting";
    stopPolling();
    try {
      await apiDisconnect();
      status.value = null;
      latencies.value = {};
      phase.value = "idle";
      error.value = null;
    } catch (e) {
      phase.value = "error";
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  function peerLatency(peer: PeerView): number | null {
    return latencies.value[peer.id] ?? peer.latencyMs ?? null;
  }

  onMounted(async () => {
    await refreshAdmin();
    try {
      unlistenAuto = await listen<{ ok: boolean; message: string }>(
        "roommate-auto-connect",
        (event) => {
          if (event.payload.ok) {
            phase.value = "connected";
            error.value = null;
            startPolling();
          } else {
            phase.value = "error";
            error.value = event.payload.message;
          }
        },
      );
    } catch {
      // not running inside tauri
    }

    // Elevated window after UAC: show connecting while Rust auto-connects.
    if (isAdmin.value) {
      const params = new URLSearchParams(window.location.search);
      // Rust may already be connecting via --roommate-connect
      void pullStatus().then(() => {
        if (status.value?.selfIps?.length) {
          phase.value = "connected";
          startPolling();
        }
      });
      void params;
    }
  });

  onUnmounted(() => {
    stopPolling();
    if (unlistenAuto) unlistenAuto();
  });

  return {
    phase,
    error,
    status,
    isAdmin,
    connect,
    disconnect,
    peerLatency,
    refreshAdmin,
  };
}

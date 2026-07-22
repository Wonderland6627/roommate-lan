import { onMounted, onUnmounted, ref, shallowRef } from "vue";
import {
  createRoom,
  dissolveRoom,
  joinRoom,
  leaveRoom,
  listMembers,
  listRooms,
  type RoomCredentials,
  type RoomMember,
  type RoomSummary,
} from "../lib/roomApi";
import {
  apiBootstrapUrl,
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
const LOBBY_MS = 5000;
const MEMBERS_MS = 3000;

const LS_DISPLAY = "roommate.displayName";
const LS_CODE = "roommate.lastCode";

export function useNetworkStatus() {
  const phase = ref<ConnectionPhase>("idle");
  const error = ref<string | null>(null);
  const status = shallowRef<NetworkStatusDto | null>(null);
  const isAdmin = ref(true);
  const latencies = ref<Record<string, number>>({});

  const bootstrapUrl = ref("");
  const rooms = ref<RoomSummary[]>([]);
  const members = ref<RoomMember[]>([]);
  const session = ref<RoomCredentials | null>(null);
  const displayName = ref(localStorage.getItem(LS_DISPLAY) ?? "");
  const roomName = ref("");
  const joinCode = ref(localStorage.getItem(LS_CODE) ?? "");
  const selectedRoomId = ref<string | null>(null);
  const busyAction = ref(false);

  let statusTimer: ReturnType<typeof setInterval> | null = null;
  let pingTimer: ReturnType<typeof setInterval> | null = null;
  let lobbyTimer: ReturnType<typeof setInterval> | null = null;
  let membersTimer: ReturnType<typeof setInterval> | null = null;

  async function refreshAdmin() {
    try {
      isAdmin.value = await apiIsAdmin();
    } catch {
      isAdmin.value = false;
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

  function stopLobbyPolling() {
    if (lobbyTimer) {
      clearInterval(lobbyTimer);
      lobbyTimer = null;
    }
  }

  function stopMembersPolling() {
    if (membersTimer) {
      clearInterval(membersTimer);
      membersTimer = null;
    }
  }

  async function pullStatus() {
    try {
      const next = await apiGetStatus();
      status.value = next;
      if (phase.value === "connected") {
        error.value = null;
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
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

  async function refreshRooms() {
    if (!bootstrapUrl.value || session.value) return;
    try {
      rooms.value = await listRooms(bootstrapUrl.value);
    } catch {
      // lobby refresh failures are non-fatal
    }
  }

  function startLobbyPolling() {
    stopLobbyPolling();
    void refreshRooms();
    lobbyTimer = setInterval(() => void refreshRooms(), LOBBY_MS);
  }

  async function refreshMembers() {
    if (!bootstrapUrl.value || !session.value) return;
    try {
      const data = await listMembers(bootstrapUrl.value, session.value.room.id);
      members.value = data.members;
      session.value = { ...session.value, room: data.room };
    } catch {
      // room may have been dissolved
    }
  }

  function startMembersPolling() {
    stopMembersPolling();
    void refreshMembers();
    membersTimer = setInterval(() => void refreshMembers(), MEMBERS_MS);
  }

  async function enterWithCredentials(creds: RoomCredentials) {
    localStorage.setItem(LS_DISPLAY, displayName.value.trim());
    localStorage.setItem(LS_CODE, creds.code);
    session.value = creds;
    joinCode.value = creds.code;
    phase.value = "connecting";
    error.value = null;
    stopLobbyPolling();
    try {
      await apiConnect({
        loginServer: creds.loginServer,
        authKey: creds.authKey,
      });
      phase.value = "connected";
      startPolling();
      startMembersPolling();
    } catch (e) {
      try {
        if (creds.isHost) {
          await dissolveRoom(bootstrapUrl.value, creds.room.id, creds.memberToken);
        } else {
          await leaveRoom(bootstrapUrl.value, creds.room.id, creds.memberToken);
        }
      } catch {
        // ignore cleanup errors
      }
      phase.value = "error";
      error.value = e instanceof Error ? e.message : String(e);
      session.value = null;
      members.value = [];
      stopMembersPolling();
      startLobbyPolling();
      await refreshAdmin();
    }
  }

  async function createAndConnect() {
    if (busyAction.value) return;
    error.value = null;
    const name = roomName.value.trim();
    const display = displayName.value.trim();
    if (!name) {
      error.value = "请填写房间名";
      return;
    }
    if (!display) {
      error.value = "请填写显示名";
      return;
    }
    if (!bootstrapUrl.value) {
      error.value = "房间服务地址未配置";
      return;
    }
    busyAction.value = true;
    phase.value = "connecting";
    try {
      const creds = await createRoom(bootstrapUrl.value, name, display);
      await enterWithCredentials(creds);
    } catch (e) {
      phase.value = "error";
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      busyAction.value = false;
    }
  }

  async function joinAndConnect() {
    if (busyAction.value) return;
    error.value = null;
    const code = joinCode.value.trim();
    const display = displayName.value.trim();
    if (!code) {
      error.value = "请填写房间码";
      return;
    }
    if (!display) {
      error.value = "请填写显示名";
      return;
    }
    if (!bootstrapUrl.value) {
      error.value = "房间服务地址未配置";
      return;
    }
    busyAction.value = true;
    phase.value = "connecting";
    try {
      const creds = await joinRoom(bootstrapUrl.value, code, display);
      await enterWithCredentials(creds);
    } catch (e) {
      phase.value = "error";
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      busyAction.value = false;
    }
  }

  function selectRoom(room: RoomSummary) {
    selectedRoomId.value = room.id;
    // Code is never listed; user must still type it.
  }

  async function leaveOrDissolve() {
    if (busyAction.value) return;
    const current = session.value;
    busyAction.value = true;
    phase.value = "disconnecting";
    stopPolling();
    stopMembersPolling();
    try {
      if (current && bootstrapUrl.value) {
        if (current.isHost) {
          await dissolveRoom(
            bootstrapUrl.value,
            current.room.id,
            current.memberToken,
          );
        } else {
          await leaveRoom(
            bootstrapUrl.value,
            current.room.id,
            current.memberToken,
          );
        }
      }
    } catch {
      // still disconnect tunnel
    }
    try {
      await apiDisconnect();
    } catch (e) {
      phase.value = "error";
      error.value = e instanceof Error ? e.message : String(e);
      busyAction.value = false;
      return;
    }
    session.value = null;
    members.value = [];
    status.value = null;
    latencies.value = {};
    phase.value = "idle";
    error.value = null;
    busyAction.value = false;
    startLobbyPolling();
  }

  /** Legacy disconnect without room API (kept for safety). */
  async function disconnect() {
    await leaveOrDissolve();
  }

  function peerLatency(peer: PeerView): number | null {
    return latencies.value[peer.id] ?? peer.latencyMs ?? null;
  }

  onMounted(async () => {
    await refreshAdmin();
    try {
      bootstrapUrl.value = await apiBootstrapUrl();
    } catch {
      bootstrapUrl.value = "";
    }
    startLobbyPolling();
    if (isAdmin.value) {
      void pullStatus().then(() => {
        if (status.value?.selfIps?.length && !session.value) {
          // Tunnel already up from previous session without room metadata.
          phase.value = "connected";
          startPolling();
        }
      });
    }
  });

  onUnmounted(() => {
    stopPolling();
    stopLobbyPolling();
    stopMembersPolling();
  });

  return {
    phase,
    error,
    status,
    isAdmin,
    rooms,
    members,
    session,
    displayName,
    roomName,
    joinCode,
    selectedRoomId,
    busyAction,
    createAndConnect,
    joinAndConnect,
    selectRoom,
    leaveOrDissolve,
    disconnect,
    peerLatency,
    refreshAdmin,
    refreshRooms,
  };
}

import { onMounted, onUnmounted, ref, shallowRef } from "vue";
import {
  createRoom,
  dissolveRoom,
  joinRoom,
  leaveOrDissolveKeepalive,
  leaveRoom,
  listMembers,
  listRooms,
  reportPresence,
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
  MemberNetInfo,
  NetworkStatusDto,
} from "../types/network";

const STATUS_MS = 2000;
const PING_MS = 5000;
const LOBBY_MS = 5000;
const MEMBERS_MS = 3000;
const PRESENCE_REFRESH_MS = 60_000;

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
  const busyAction = ref(false);
  const lastReportedKey = ref("");
  const lastPresenceAt = ref(0);

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

  async function maybeReportPresence() {
    const current = session.value;
    const st = status.value;
    if (!current || !bootstrapUrl.value || !st) return;
    if (phase.value !== "connected") return;

    const nodeId = st.selfId?.trim();
    const virtualIp = st.selfIps?.[0]?.trim();
    if (!nodeId || !virtualIp) return;

    const key = `${current.room.id}:${nodeId}:${virtualIp}`;
    const now = Date.now();
    const freshEnough =
      key === lastReportedKey.value &&
      now - lastPresenceAt.value < PRESENCE_REFRESH_MS;
    if (freshEnough) return;

    try {
      await reportPresence(bootstrapUrl.value, current.room.id, {
        memberToken: current.memberToken,
        nodeId,
        virtualIp,
      });
      lastReportedKey.value = key;
      lastPresenceAt.value = now;
    } catch {
      // presence is best-effort; members poll will stay without IP until retry
    }
  }

  async function pullStatus() {
    try {
      const next = await apiGetStatus();
      status.value = next;
      if (phase.value === "connected") {
        error.value = null;
        await maybeReportPresence();
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (phase.value === "connected") {
        error.value = msg;
      }
    }
  }

  async function pullPings() {
    const current = session.value;
    if (!current || phase.value !== "connected") return;

    const targets = members.value.filter(
      (m) =>
        m.id !== current.memberId &&
        !!m.virtualIp &&
        m.virtualIp.trim().length > 0,
    );

    await Promise.all(
      targets.map(async (member) => {
        const ip = member.virtualIp!.trim();
        try {
          const ms = await apiPingPeer(ip);
          latencies.value = { ...latencies.value, [member.id]: ms };
        } catch {
          // ignore single-member ping failures
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
    lastReportedKey.value = "";
    lastPresenceAt.value = 0;
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
      lastReportedKey.value = "";
      lastPresenceAt.value = 0;
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

  async function leaveOrDissolve(opts?: { force?: boolean }) {
    if (busyAction.value && !opts?.force) return;
    const current = session.value;
    if (!current) {
      if (opts?.force) {
        try {
          await apiDisconnect();
        } catch {
          // ignore
        }
      }
      return;
    }

    busyAction.value = true;
    phase.value = "disconnecting";
    stopPolling();
    stopMembersPolling();

    let roomErr: string | null = null;
    try {
      if (bootstrapUrl.value) {
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
    } catch (e) {
      roomErr = e instanceof Error ? e.message : String(e);
      if (!opts?.force) {
        error.value = `退出房间失败（房间可能仍留在列表）: ${roomErr}`;
        phase.value = "connected";
        busyAction.value = false;
        startPolling();
        startMembersPolling();
        return;
      }
    }

    try {
      await apiDisconnect();
    } catch (e) {
      if (!opts?.force) {
        phase.value = "error";
        error.value = e instanceof Error ? e.message : String(e);
        busyAction.value = false;
        return;
      }
    }

    session.value = null;
    members.value = [];
    status.value = null;
    latencies.value = {};
    lastReportedKey.value = "";
    lastPresenceAt.value = 0;
    phase.value = "idle";
    error.value = roomErr
      ? `已断开，但房间清理失败（可能仍在列表）: ${roomErr}`
      : null;
    busyAction.value = false;
    startLobbyPolling();
  }

  /** Non-blocking cleanup for window close — never blocks the OS close button. */
  function cleanupOnWindowClose() {
    const current = session.value;
    const base = bootstrapUrl.value;
    if (current && base) {
      leaveOrDissolveKeepalive(
        base,
        current.room.id,
        current.memberToken,
        current.isHost,
      );
    }
    session.value = null;
    // Tunnel teardown is handled by Rust RunEvent::Exit → service disconnect.
  }

  /** Legacy disconnect without room API (kept for safety). */
  async function disconnect() {
    await leaveOrDissolve();
  }

  function memberNet(member: RoomMember): MemberNetInfo {
    const current = session.value;
    const selfIps = status.value?.selfIps ?? [];
    const vip = member.virtualIp?.trim() || "";
    const isSelf =
      !!current &&
      ((!!current.memberId && member.id === current.memberId) ||
        (!!vip && selfIps.includes(vip)));
    const virtualIp = vip || null;

    if (isSelf) {
      return {
        kind: "self",
        latencyMs: null,
        virtualIp: virtualIp ?? status.value?.selfIps?.[0] ?? null,
        isSelf: true,
      };
    }

    if (!virtualIp && !member.nodeId) {
      return {
        kind: "pending",
        latencyMs: null,
        virtualIp: null,
        isSelf: false,
      };
    }

    const peers = status.value?.peers ?? [];
    const peer =
      peers.find((p) => member.nodeId && p.id === member.nodeId) ??
      peers.find((p) => virtualIp && p.ips.includes(virtualIp));

    if (!peer) {
      return {
        kind: virtualIp ? "idle" : "pending",
        latencyMs: latencies.value[member.id] ?? null,
        virtualIp,
        isSelf: false,
      };
    }

    return {
      kind: peer.conn,
      relay: peer.relay,
      latencyMs: latencies.value[member.id] ?? peer.latencyMs ?? null,
      virtualIp: virtualIp ?? peer.ips[0] ?? null,
      isSelf: false,
    };
  }

  onMounted(async () => {
    await refreshAdmin();
    try {
      bootstrapUrl.value = await apiBootstrapUrl();
    } catch {
      bootstrapUrl.value = "";
    }
    startLobbyPolling();

    // Do NOT use Tauri onCloseRequested + preventDefault — it can stick the
    // window closed permanently (especially across Vite HMR remounts).
    // Keepalive dissolve/leave is best-effort; Rust Exit disconnects the tunnel.
    window.addEventListener("pagehide", cleanupOnWindowClose);
  });

  onUnmounted(() => {
    stopPolling();
    stopLobbyPolling();
    stopMembersPolling();
    window.removeEventListener("pagehide", cleanupOnWindowClose);
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
    busyAction,
    createAndConnect,
    joinAndConnect,
    leaveOrDissolve,
    disconnect,
    memberNet,
    refreshAdmin,
    refreshRooms,
  };
}

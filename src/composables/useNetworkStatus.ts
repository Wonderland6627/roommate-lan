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
  type TrafficReport,
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
  PeerView,
} from "../types/network";

const STATUS_MS = 2000;
const PING_MS = 5000;
const LOBBY_MS = 5000;
const MEMBERS_MS = 3000;
const PRESENCE_REFRESH_MS = 60_000;
/** Must exceed service-side tailscale up (~45s) + Running/IP wait. */
const CONNECT_TIMEOUT_MS = 60_000;
const ROOM_CLEANUP_RETRIES = 3;

const LS_DISPLAY = "roommate.displayName";
const LS_CODE = "roommate.lastCode";

type PeerByteSample = { tx: number; rx: number };

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
  const joinCode = ref(
    (localStorage.getItem(LS_CODE) ?? "")
      .toUpperCase()
      .replace(/[^A-Z]/g, "")
      .slice(0, 4),
  );
  const busyAction = ref(false);
  const lastReportedKey = ref("");
  const lastPresenceAt = ref(0);

  /** Session-local path-classified traffic (not reactive UI state). */
  let relayBytesAccum = 0;
  let p2pBytesAccum = 0;
  const lastPeerBytes = new Map<string, PeerByteSample>();

  let statusTimer: ReturnType<typeof setInterval> | null = null;
  let pingTimer: ReturnType<typeof setInterval> | null = null;
  let lobbyTimer: ReturnType<typeof setInterval> | null = null;
  let membersTimer: ReturnType<typeof setInterval> | null = null;

  function resetTrafficAccum() {
    relayBytesAccum = 0;
    p2pBytesAccum = 0;
    lastPeerBytes.clear();
  }

  function withTimeout<T>(
    promise: Promise<T>,
    ms: number,
    message: string,
  ): Promise<T> {
    return new Promise((resolve, reject) => {
      const timer = window.setTimeout(() => reject(new Error(message)), ms);
      promise.then(
        (value) => {
          window.clearTimeout(timer);
          resolve(value);
        },
        (err) => {
          window.clearTimeout(timer);
          reject(err);
        },
      );
    });
  }

  function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => {
      window.setTimeout(resolve, ms);
    });
  }

  async function withRetries(
    action: () => Promise<void>,
    attempts: number,
  ): Promise<void> {
    let lastError: unknown;
    for (let i = 0; i < attempts; i++) {
      try {
        await action();
        return;
      } catch (e) {
        lastError = e;
        if (i + 1 < attempts) {
          await sleep(300 * (i + 1));
        }
      }
    }
    throw lastError instanceof Error
      ? lastError
      : new Error(String(lastError));
  }

  async function cleanupRoomSeat(
    creds: RoomCredentials,
    traffic: TrafficReport,
  ): Promise<void> {
    if (!bootstrapUrl.value) return;
    await withRetries(async () => {
      if (creds.isHost) {
        await dissolveRoom(
          bootstrapUrl.value,
          creds.room.id,
          creds.memberToken,
          traffic,
        );
      } else {
        await leaveRoom(
          bootstrapUrl.value,
          creds.room.id,
          creds.memberToken,
          traffic,
        );
      }
    }, ROOM_CLEANUP_RETRIES);
  }

  function currentTraffic(): TrafficReport {
    return {
      relayBytes: Math.max(0, Math.floor(relayBytesAccum)),
      p2pBytes: Math.max(0, Math.floor(p2pBytesAccum)),
    };
  }

  function findMemberPeer(
    member: RoomMember,
    peers: PeerView[],
  ): PeerView | undefined {
    const vip = member.virtualIp?.trim() || "";
    return (
      peers.find((p) => member.nodeId && p.id === member.nodeId) ??
      peers.find((p) => vip && p.ips.includes(vip))
    );
  }

  function accumulateTraffic(st: NetworkStatusDto) {
    const current = session.value;
    if (!current || phase.value !== "connected") return;

    const selfMemberId = current.memberId;
    const roomPeers = members.value.filter((m) => m.id !== selfMemberId);

    for (const member of roomPeers) {
      const peer = findMemberPeer(member, st.peers ?? []);
      if (!peer) continue;

      const tx = typeof peer.txBytes === "number" ? peer.txBytes : 0;
      const rx = typeof peer.rxBytes === "number" ? peer.rxBytes : 0;
      const total = Math.max(0, tx) + Math.max(0, rx);
      const key = peer.id || member.id;
      const prev = lastPeerBytes.get(key);
      lastPeerBytes.set(key, { tx, rx });
      if (!prev) continue;

      const prevTotal = Math.max(0, prev.tx) + Math.max(0, prev.rx);
      const delta = total - prevTotal;
      if (delta <= 0) continue;

      if (peer.conn === "derpRelay") {
        relayBytesAccum += delta;
      } else if (peer.conn === "p2p" || peer.conn === "peerRelay") {
        p2pBytesAccum += delta;
      }
    }
  }

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
        traffic: currentTraffic(),
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
        accumulateTraffic(next);
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
    resetTrafficAccum();
    stopLobbyPolling();
    try {
      await withTimeout(
        apiConnect({
          loginServer: creds.loginServer,
          authKey: creds.authKey,
        }),
        CONNECT_TIMEOUT_MS,
        "隧道建立超时，请稍后重试",
      );
      phase.value = "connected";
      startPolling();
      startMembersPolling();
    } catch (e) {
      const connectMsg = e instanceof Error ? e.message : String(e);
      let cleanupFailed = false;
      try {
        await cleanupRoomSeat(creds, currentTraffic());
      } catch {
        cleanupFailed = true;
      }
      try {
        await apiDisconnect();
      } catch {
        // best-effort local teardown after failed connect
      }
      phase.value = "error";
      error.value = cleanupFailed
        ? `${connectMsg}（房间席位可能仍占用，请稍后再试或换显示名）`
        : connectMsg;
      session.value = null;
      members.value = [];
      lastReportedKey.value = "";
      lastPresenceAt.value = 0;
      resetTrafficAccum();
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
    const code = joinCode.value
      .toUpperCase()
      .replace(/[^A-Z]/g, "")
      .slice(0, 4);
    joinCode.value = code;
    const display = displayName.value.trim();
    if (!code) {
      error.value = "请填写房间码";
      return;
    }
    if (code.length !== 4) {
      error.value = "房间码须为 4 位字母";
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

    // One last status sample so leave/dissolve carries freshest deltas.
    try {
      const next = await apiGetStatus();
      status.value = next;
      accumulateTraffic(next);
    } catch {
      // best-effort
    }
    const traffic = currentTraffic();

    let roomErr: string | null = null;
    try {
      if (bootstrapUrl.value) {
        await cleanupRoomSeat(current, traffic);
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
      await withTimeout(
        apiDisconnect(),
        30_000,
        "断开隧道超时，请稍后重试或重启应用",
      );
    } catch (e) {
      if (!opts?.force) {
        // Room seat already cleared; drop local session so UI is not stuck in transit.
        session.value = null;
        members.value = [];
        status.value = null;
        latencies.value = {};
        lastReportedKey.value = "";
        lastPresenceAt.value = 0;
        resetTrafficAccum();
        phase.value = "error";
        error.value = e instanceof Error ? e.message : String(e);
        busyAction.value = false;
        startLobbyPolling();
        return;
      }
    }

    session.value = null;
    members.value = [];
    status.value = null;
    latencies.value = {};
    lastReportedKey.value = "";
    lastPresenceAt.value = 0;
    resetTrafficAccum();
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
        currentTraffic(),
      );
    }
    session.value = null;
    resetTrafficAccum();
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
    const peer = findMemberPeer(member, peers);

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

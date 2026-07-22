<script setup lang="ts">
import { computed } from "vue";
import RoomLobby from "./components/RoomLobby.vue";
import RoomSession from "./components/RoomSession.vue";
import UpdateStatus from "./components/UpdateStatus.vue";
import { useNetworkStatus } from "./composables/useNetworkStatus";

const {
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
  memberNet,
  refreshRooms,
} = useNetworkStatus();

const busy = computed(
  () =>
    busyAction.value ||
    phase.value === "connecting" ||
    phase.value === "disconnecting",
);

const inRoom = computed(() => !!session.value && phase.value === "connected");
const showLobby = computed(
  () =>
    !session.value &&
    (phase.value === "idle" || phase.value === "error"),
);
const showTransit = computed(
  () =>
    phase.value === "connecting" || phase.value === "disconnecting",
);

const selfIp = computed(() => status.value?.selfIps?.[0] ?? "");
const backendLabel = computed(() => {
  if (!isAdmin.value) return "服务未就绪";
  return status.value?.backendState ?? (inRoom.value ? "…" : "Idle");
});
</script>

<template>
  <div class="shell">
    <header class="topbar animate-fade-up">
      <p class="brand">Roommate-LAN</p>
      <div class="self-status">
        <div class="ip">{{ selfIp || "未分配 IP" }}</div>
        <div class="state">
          {{ backendLabel }}
          <span v-if="!isAdmin" class="warn"> · 请检查安装</span>
        </div>
      </div>
    </header>

    <RoomLobby
      v-if="showLobby"
      class="animate-fade-up"
      style="animation-delay: 0.05s"
      :rooms="rooms"
      :room-name="roomName"
      :display-name="displayName"
      :join-code="joinCode"
      :busy="busy"
      @update:room-name="roomName = $event"
      @update:display-name="displayName = $event"
      @update:join-code="joinCode = $event"
      @create="createAndConnect"
      @join="joinAndConnect"
      @refresh="refreshRooms"
    />

    <div
      v-else-if="showTransit"
      class="transit animate-fade-up"
      style="animation-delay: 0.05s"
    >
      <p class="transit-title">
        {{ phase === "disconnecting" ? "正在退出…" : "正在连接…" }}
      </p>
      <p class="transit-hint">
        {{
          phase === "disconnecting"
            ? "正在清理房间并断开隧道，请稍候"
            : "隧道建立中，请稍候"
        }}
      </p>
    </div>

    <RoomSession
      v-else-if="inRoom && session"
      class="animate-fade-up"
      style="animation-delay: 0.05s"
      :session="session"
      :members="members"
      :busy="busy"
      :net-of="memberNet"
      @leave="leaveOrDissolve"
    />

    <p v-if="error" class="error animate-fade-up" role="alert">{{ error }}</p>

    <UpdateStatus class="animate-fade-up" style="animation-delay: 0.15s" />
  </div>
</template>

<style scoped>
.shell {
  min-height: 100%;
  max-width: 420px;
  margin: 0 auto;
  padding: 1.75rem 1.35rem 2rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.topbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  margin-bottom: 0.15rem;
}
.brand {
  margin: 0;
  font-family: var(--font-display);
  font-size: 1.85rem;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.1;
  background: linear-gradient(120deg, #e8f0ec 20%, var(--accent) 90%);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}
.self-status {
  text-align: right;
  flex-shrink: 0;
  padding-top: 0.15rem;
}
.self-status .ip {
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
  font-size: 0.85rem;
  color: var(--accent);
}
.self-status .state {
  margin-top: 0.15rem;
  font-size: 0.68rem;
  color: var(--ink-muted);
}
.warn {
  color: var(--warn);
}
.transit {
  padding: 1.75rem 1rem;
  border-radius: 12px;
  text-align: center;
  background: color-mix(in srgb, var(--panel) 88%, transparent);
  box-shadow: inset 0 0 0 1px var(--line);
}
.transit-title {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
}
.transit-hint {
  margin: 0.45rem 0 0;
  font-size: 0.8rem;
  color: var(--ink-muted);
}
.error {
  margin: 0;
  padding: 0.7rem 0.85rem;
  border-radius: 10px;
  font-size: 0.8rem;
  line-height: 1.4;
  color: #1a0e0c;
  background: color-mix(in srgb, var(--danger) 85%, white);
}
</style>

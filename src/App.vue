<script setup lang="ts">
import { computed } from "vue";
import PeerList from "./components/PeerList.vue";
import RoomLobby from "./components/RoomLobby.vue";
import RoomSession from "./components/RoomSession.vue";
import StatusBar from "./components/StatusBar.vue";
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
  selectedRoomId,
  busyAction,
  createAndConnect,
  joinAndConnect,
  selectRoom,
  leaveOrDissolve,
  peerLatency,
  refreshRooms,
} = useNetworkStatus();

const busy = computed(
  () =>
    busyAction.value ||
    phase.value === "connecting" ||
    phase.value === "disconnecting",
);

const inRoom = computed(() => !!session.value && phase.value === "connected");
</script>

<template>
  <div class="shell">
    <header class="hero animate-fade-up">
      <p class="brand">Roommate-LAN</p>
    </header>

    <RoomLobby
      v-if="!inRoom"
      class="animate-fade-up"
      style="animation-delay: 0.05s"
      :rooms="rooms"
      :room-name="roomName"
      :display-name="displayName"
      :join-code="joinCode"
      :selected-room-id="selectedRoomId"
      :busy="busy"
      @update:room-name="roomName = $event"
      @update:display-name="displayName = $event"
      @update:join-code="joinCode = $event"
      @select="selectRoom"
      @create="createAndConnect"
      @join="joinAndConnect"
      @refresh="refreshRooms"
    />

    <RoomSession
      v-else-if="session"
      class="animate-fade-up"
      style="animation-delay: 0.05s"
      :session="session"
      :members="members"
      :busy="busy"
      @leave="leaveOrDissolve"
    />

    <p v-if="error" class="error animate-fade-up" role="alert">{{ error }}</p>

    <StatusBar
      class="animate-fade-up"
      style="animation-delay: 0.1s"
      :hostname="status?.selfHostname ?? ''"
      :ips="status?.selfIps ?? []"
      :backend-state="status?.backendState ?? (phase === 'connected' ? '…' : 'Idle')"
      :is-admin="isAdmin"
    />

    <PeerList
      v-if="phase === 'connected' || status"
      class="animate-fade-up"
      style="animation-delay: 0.15s"
      :peers="status?.peers ?? []"
      :latency-of="peerLatency"
    />

    <UpdateStatus class="animate-fade-up" style="animation-delay: 0.2s" />
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
.hero {
  margin-bottom: 0.25rem;
}
.brand {
  margin: 0;
  font-family: var(--font-display);
  font-size: 2.15rem;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.1;
  background: linear-gradient(120deg, #e8f0ec 20%, var(--accent) 90%);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
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

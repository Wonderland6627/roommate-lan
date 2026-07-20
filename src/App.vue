<script setup lang="ts">
import { computed } from "vue";
import ConnectButton from "./components/ConnectButton.vue";
import PeerList from "./components/PeerList.vue";
import StatusBar from "./components/StatusBar.vue";
import { useNetworkStatus } from "./composables/useNetworkStatus";

const { phase, error, status, isAdmin, connect, disconnect, peerLatency } =
  useNetworkStatus();

const busy = computed(
  () => phase.value === "connecting" || phase.value === "disconnecting",
);
</script>

<template>
  <div class="shell">
    <header class="hero animate-fade-up">
      <p class="brand">Roommate-LAN</p>
      <p class="tagline">一键进入虚拟局域网，Steam 联机少卡顿</p>
    </header>

    <ConnectButton
      class="animate-fade-up"
      style="animation-delay: 0.05s"
      :phase="phase"
      :busy="busy"
      @connect="connect"
      @disconnect="disconnect"
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
.tagline {
  margin: 0.45rem 0 0;
  color: var(--ink-muted);
  font-size: 0.9rem;
  line-height: 1.45;
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

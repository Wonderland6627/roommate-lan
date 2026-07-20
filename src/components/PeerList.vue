<script setup lang="ts">
import PeerRow from "./PeerRow.vue";
import type { PeerView } from "../types/network";

defineProps<{
  peers: PeerView[];
  latencyOf: (peer: PeerView) => number | null;
}>();
</script>

<template>
  <section class="list-wrap">
    <header class="head">
      <h2>队友</h2>
      <span class="count">{{ peers.length }}</span>
    </header>
    <ul v-if="peers.length" class="list">
      <PeerRow
        v-for="peer in peers"
        :key="peer.id"
        :peer="peer"
        :latency-ms="latencyOf(peer)"
      />
    </ul>
    <p v-else class="empty">房间里还没有其他队友在线</p>
  </section>
</template>

<style scoped>
.list-wrap {
  margin-top: 0.5rem;
}
.head {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  margin-bottom: 0.25rem;
}
.head h2 {
  margin: 0;
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--ink-muted);
}
.count {
  font-size: 0.7rem;
  color: var(--accent);
}
.list {
  list-style: none;
  margin: 0;
  padding: 0;
}
.empty {
  margin: 1rem 0 0;
  color: var(--ink-muted);
  font-size: 0.85rem;
}
</style>

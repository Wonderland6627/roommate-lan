<script setup lang="ts">
import type { ConnectionPhase } from "../types/network";

defineProps<{
  phase: ConnectionPhase;
  busy: boolean;
}>();

const emit = defineEmits<{
  connect: [];
  disconnect: [];
}>();
</script>

<template>
  <button
    type="button"
    class="cta"
    :class="{
      connected: phase === 'connected',
      busy,
      pulse: phase === 'connecting',
    }"
    :disabled="busy"
    @click="phase === 'connected' ? emit('disconnect') : emit('connect')"
  >
    <span v-if="phase === 'connecting'">连接中…</span>
    <span v-else-if="phase === 'disconnecting'">断开中…</span>
    <span v-else-if="phase === 'connected'">断开连接</span>
    <span v-else>一键连接</span>
  </button>
</template>

<style scoped>
.cta {
  width: 100%;
  border: none;
  border-radius: 12px;
  padding: 0.95rem 1.25rem;
  font-size: 1rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  cursor: pointer;
  color: #0c1210;
  background: linear-gradient(135deg, var(--accent) 0%, var(--accent-dim) 100%);
  transition: transform 0.15s ease, filter 0.2s ease, background 0.25s ease;
}
.cta:hover:not(:disabled) {
  filter: brightness(1.06);
  transform: translateY(-1px);
}
.cta:active:not(:disabled) {
  transform: translateY(0);
}
.cta:disabled {
  opacity: 0.7;
  cursor: wait;
}
.cta.connected {
  color: var(--ink);
  background: rgba(232, 240, 236, 0.1);
  box-shadow: inset 0 0 0 1px var(--line);
}
.cta.pulse {
  animation: pulse-soft 1.8s ease-in-out infinite;
}
</style>

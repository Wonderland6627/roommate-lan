<script setup lang="ts">
import { ref } from "vue";
import ConnBadge from "./ConnBadge.vue";
import { copyText } from "../composables/useClipboard";
import type { PeerView } from "../types/network";

const props = defineProps<{
  peer: PeerView;
  latencyMs: number | null;
}>();

const copied = ref(false);

async function onCopy() {
  const ip = props.peer.ips[0];
  if (!ip) return;
  await copyText(ip);
  copied.value = true;
  setTimeout(() => {
    copied.value = false;
  }, 1200);
}
</script>

<template>
  <li class="row animate-fade-up">
    <div class="meta">
      <div class="name">{{ peer.hostname }}</div>
      <button
        type="button"
        class="ip"
        :title="copied ? '已复制' : '点击复制 IP'"
        :disabled="!peer.ips[0]"
        @click="onCopy"
      >
        {{ peer.ips[0] ?? "—" }}
        <span class="hint">{{ copied ? "已复制" : "复制" }}</span>
      </button>
    </div>
    <div class="side">
      <ConnBadge :kind="peer.conn" :relay="peer.relay" />
      <span class="rtt" :data-empty="latencyMs == null">
        {{ latencyMs != null ? `${latencyMs} ms` : "…" }}
      </span>
    </div>
  </li>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.85rem 0;
  border-bottom: 1px solid var(--line);
}
.row:last-child {
  border-bottom: none;
}
.meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}
.name {
  font-weight: 600;
  font-size: 0.95rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ip {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--ink-muted);
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
  font-size: 0.8rem;
  padding: 0;
  cursor: pointer;
  text-align: left;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
}
.ip:hover:not(:disabled) {
  color: var(--accent);
}
.ip:disabled {
  cursor: default;
}
.hint {
  font-size: 0.65rem;
  opacity: 0.7;
  font-family: var(--font-body);
}
.side {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 0.35rem;
  flex-shrink: 0;
}
.rtt {
  font-variant-numeric: tabular-nums;
  font-size: 0.75rem;
  color: var(--ink-muted);
}
.rtt[data-empty="true"] {
  opacity: 0.45;
}
</style>

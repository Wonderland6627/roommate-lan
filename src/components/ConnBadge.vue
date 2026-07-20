<script setup lang="ts">
import type { ConnKind } from "../types/network";

const props = defineProps<{
  kind: ConnKind;
  relay?: string | null;
}>();

const label = () => {
  switch (props.kind) {
    case "p2p":
      return "P2P 直连";
    case "derpRelay":
      return props.relay ? `腾讯云 DERP · ${props.relay}` : "腾讯云 DERP";
    case "peerRelay":
      return "Peer 中继";
    case "idle":
      return "空闲";
    case "offline":
      return "离线";
    default:
      return "未知";
  }
};

const tone = () => {
  switch (props.kind) {
    case "p2p":
      return "badge-p2p";
    case "derpRelay":
    case "peerRelay":
      return "badge-derp";
    case "offline":
      return "badge-off";
    default:
      return "badge-muted";
  }
};
</script>

<template>
  <span class="badge" :class="tone()">
    <span class="dot" aria-hidden="true" />
    {{ label() }}
  </span>
</template>

<style scoped>
.badge {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.7rem;
  font-weight: 600;
  letter-spacing: 0.02em;
  padding: 0.2rem 0.55rem;
  border-radius: 999px;
  transition: background 0.25s ease, color 0.25s ease;
}
.dot {
  width: 0.4rem;
  height: 0.4rem;
  border-radius: 50%;
  background: currentColor;
}
.badge-p2p {
  color: #0c1210;
  background: var(--p2p);
}
.badge-derp {
  color: #0c1210;
  background: var(--derp);
}
.badge-off {
  color: var(--ink-muted);
  background: rgba(232, 240, 236, 0.08);
}
.badge-muted {
  color: var(--warn);
  background: rgba(232, 184, 74, 0.15);
}
</style>

<script setup lang="ts">
defineProps<{
  hostname: string;
  ips: string[];
  backendState: string;
  isAdmin: boolean;
}>();
</script>

<template>
  <div class="bar">
    <div>
      <div class="label">本机</div>
      <div class="host">{{ hostname || "—" }}</div>
    </div>
    <div class="right">
      <div class="ip">{{ ips[0] || "未分配 IP" }}</div>
      <div class="state">
        {{ backendState || "Idle" }}
        <span v-if="!isAdmin" class="warn">· 需管理员</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.bar {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1rem;
  border-radius: 12px;
  background: var(--bg-panel);
  backdrop-filter: blur(10px);
  box-shadow: inset 0 0 0 1px var(--line);
}
.label {
  font-size: 0.65rem;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--ink-muted);
}
.host {
  font-weight: 600;
  margin-top: 0.15rem;
}
.right {
  text-align: right;
}
.ip {
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
  font-size: 0.9rem;
  color: var(--accent);
}
.state {
  margin-top: 0.2rem;
  font-size: 0.7rem;
  color: var(--ink-muted);
}
.warn {
  color: var(--warn);
}
</style>

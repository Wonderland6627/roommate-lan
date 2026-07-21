<script setup lang="ts">
import { computed } from "vue";
import { useUpdater } from "../composables/useUpdater";

const {
  currentVersion,
  phase,
  availableVersion,
  notes,
  statusText,
  checkForUpdates,
  downloadAndInstall,
} = useUpdater();

const busy = computed(
  () =>
    phase.value === "checking" ||
    phase.value === "downloading" ||
    phase.value === "restarting",
);
</script>

<template>
  <footer class="update">
    <div class="row">
      <span class="version">v{{ currentVersion || "…" }}</span>
      <button
        type="button"
        class="link"
        :disabled="busy"
        @click="checkForUpdates()"
      >
        检查更新
      </button>
    </div>

    <p v-if="statusText" class="status" :class="{ danger: phase === 'error' }">
      {{ statusText }}
    </p>

    <div v-if="phase === 'available'" class="actions">
      <p v-if="notes" class="notes">{{ notes }}</p>
      <button type="button" class="install" @click="downloadAndInstall">
        下载并安装 v{{ availableVersion }}
      </button>
    </div>
  </footer>
</template>

<style scoped>
.update {
  margin-top: auto;
  padding-top: 0.5rem;
  border-top: 1px solid var(--line);
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}
.version {
  font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
  font-size: 0.75rem;
  color: var(--ink-muted);
}
.link,
.install {
  border: 0;
  cursor: pointer;
  font: inherit;
}
.link {
  background: transparent;
  color: var(--accent);
  font-size: 0.75rem;
  padding: 0.15rem 0;
}
.link:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.status {
  margin: 0;
  font-size: 0.72rem;
  line-height: 1.4;
  color: var(--ink-muted);
}
.status.danger {
  color: var(--danger);
}
.actions {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}
.notes {
  margin: 0;
  font-size: 0.7rem;
  line-height: 1.4;
  color: var(--ink-muted);
  white-space: pre-wrap;
  max-height: 4.5rem;
  overflow: auto;
}
.install {
  align-self: flex-start;
  padding: 0.45rem 0.8rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--accent) 88%, black);
  color: #06140e;
  font-size: 0.78rem;
  font-weight: 600;
}
.install:hover {
  filter: brightness(1.05);
}
</style>

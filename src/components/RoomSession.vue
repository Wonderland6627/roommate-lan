<script setup lang="ts">
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ref } from "vue";
import type { RoomCredentials, RoomMember } from "../lib/roomApi";

const props = defineProps<{
  session: RoomCredentials;
  members: RoomMember[];
  busy: boolean;
}>();

const emit = defineEmits<{
  leave: [];
}>();

const copied = ref(false);

async function copyCode() {
  try {
    await writeText(props.session.code);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch {
    copied.value = false;
  }
}
</script>

<template>
  <section class="session">
    <div class="room-card">
      <p class="label">当前房间</p>
      <p class="name">{{ session.room.name }}</p>
      <div class="code-row">
        <span class="code">{{ session.code }}</span>
        <button type="button" class="copy" @click="copyCode">
          {{ copied ? "已复制" : "复制" }}
        </button>
      </div>
      <p class="hint">把房间码发给队友；列表里选房后仍需输入此码</p>
    </div>

    <div class="members">
      <header class="head">
        <h2>房间成员</h2>
        <span class="count">{{ members.length }}</span>
      </header>
      <ul v-if="members.length">
        <li v-for="m in members" :key="m.id">
          <span>{{ m.displayName }}</span>
          <span v-if="m.isHost" class="badge">房主</span>
        </li>
      </ul>
      <p v-else class="empty">暂无成员信息</p>
    </div>

    <button
      type="button"
      class="leave"
      :disabled="busy"
      @click="emit('leave')"
    >
      {{
        busy
          ? "处理中…"
          : session.isHost
            ? "解散房间"
            : "退出房间"
      }}
    </button>
  </section>
</template>

<style scoped>
.session {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}
.room-card {
  padding: 0.9rem 1rem;
  border-radius: 12px;
  background: color-mix(in srgb, var(--panel) 88%, transparent);
  box-shadow: inset 0 0 0 1px var(--line);
}
.label {
  margin: 0;
  font-size: 0.75rem;
  color: var(--ink-muted);
}
.name {
  margin: 0.25rem 0 0.65rem;
  font-size: 1.15rem;
  font-weight: 700;
}
.code-row {
  display: flex;
  align-items: center;
  gap: 0.65rem;
}
.code {
  font-family: var(--font-display);
  font-size: 1.6rem;
  letter-spacing: 0.28em;
  font-weight: 700;
  color: var(--accent);
}
.copy {
  border: none;
  border-radius: 8px;
  padding: 0.35rem 0.65rem;
  font-size: 0.75rem;
  cursor: pointer;
  color: var(--ink);
  background: rgba(232, 240, 236, 0.1);
  box-shadow: inset 0 0 0 1px var(--line);
}
.hint {
  margin: 0.55rem 0 0;
  font-size: 0.72rem;
  color: var(--ink-muted);
  line-height: 1.4;
}
.members {
  padding: 0.75rem 0.85rem;
  border-radius: 12px;
  background: color-mix(in srgb, var(--panel) 88%, transparent);
  box-shadow: inset 0 0 0 1px var(--line);
}
.head {
  display: flex;
  align-items: baseline;
  gap: 0.45rem;
  margin-bottom: 0.45rem;
}
.head h2 {
  margin: 0;
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--ink-muted);
}
.count {
  font-size: 0.75rem;
  color: var(--accent);
}
.members ul {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}
.members li {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.9rem;
}
.badge {
  font-size: 0.7rem;
  color: var(--accent);
}
.empty {
  margin: 0;
  font-size: 0.8rem;
  color: var(--ink-muted);
}
.leave {
  width: 100%;
  border: none;
  border-radius: 12px;
  padding: 0.85rem 1.1rem;
  font-size: 0.95rem;
  font-weight: 700;
  cursor: pointer;
  color: var(--ink);
  background: rgba(232, 240, 236, 0.1);
  box-shadow: inset 0 0 0 1px var(--line);
}
.leave:disabled {
  opacity: 0.7;
  cursor: wait;
}
</style>

<script setup lang="ts">
import type { RoomSummary } from "../lib/roomApi";

defineProps<{
  rooms: RoomSummary[];
  roomName: string;
  displayName: string;
  joinCode: string;
  selectedRoomId: string | null;
  busy: boolean;
}>();

const emit = defineEmits<{
  "update:roomName": [string];
  "update:displayName": [string];
  "update:joinCode": [string];
  select: [RoomSummary];
  create: [];
  join: [];
  refresh: [];
}>();
</script>

<template>
  <section class="lobby">
    <label class="field">
      <span>显示名</span>
      <input
        :value="displayName"
        maxlength="16"
        placeholder="在房间里怎么称呼你"
        :disabled="busy"
        @input="emit('update:displayName', ($event.target as HTMLInputElement).value)"
      />
    </label>

    <div class="panel">
      <h2>创建房间</h2>
      <label class="field">
        <span>房间名</span>
        <input
          :value="roomName"
          maxlength="32"
          placeholder="今晚联机"
          :disabled="busy"
          @input="emit('update:roomName', ($event.target as HTMLInputElement).value)"
        />
      </label>
      <button type="button" class="cta" :disabled="busy" @click="emit('create')">
        {{ busy ? "处理中…" : "创建房间" }}
      </button>
    </div>

    <div class="panel">
      <div class="panel-head">
        <h2>房间列表</h2>
        <button type="button" class="linkish" :disabled="busy" @click="emit('refresh')">
          刷新
        </button>
      </div>
      <ul v-if="rooms.length" class="rooms">
        <li
          v-for="room in rooms"
          :key="room.id"
          :class="{ selected: selectedRoomId === room.id }"
          @click="emit('select', room)"
        >
          <span class="rname">{{ room.name }}</span>
          <span class="meta">{{ room.memberCount }} 人</span>
        </li>
      </ul>
      <p v-else class="empty">暂无房间，创建一个吧</p>

      <label class="field">
        <span>房间码（4 位字母）</span>
        <input
          :value="joinCode"
          maxlength="8"
          placeholder="ABCD"
          class="code"
          :disabled="busy"
          @input="emit('update:joinCode', ($event.target as HTMLInputElement).value.toUpperCase())"
        />
      </label>
      <button type="button" class="cta secondary" :disabled="busy" @click="emit('join')">
        {{ busy ? "处理中…" : "加入房间" }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.lobby {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}
.panel {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
  padding: 0.85rem 0.9rem;
  border-radius: 12px;
  background: color-mix(in srgb, var(--panel) 88%, transparent);
  box-shadow: inset 0 0 0 1px var(--line);
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.panel h2 {
  margin: 0;
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--ink-muted);
  letter-spacing: 0.04em;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  font-size: 0.75rem;
  color: var(--ink-muted);
}
.field input {
  border: none;
  border-radius: 10px;
  padding: 0.65rem 0.75rem;
  font-size: 0.95rem;
  color: var(--ink);
  background: rgba(0, 0, 0, 0.25);
  box-shadow: inset 0 0 0 1px var(--line);
}
.field input.code {
  letter-spacing: 0.28em;
  text-transform: uppercase;
  font-weight: 700;
}
.field input:focus {
  outline: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
  outline-offset: 1px;
}
.rooms {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  max-height: 9rem;
  overflow: auto;
}
.rooms li {
  display: flex;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.55rem 0.65rem;
  border-radius: 8px;
  cursor: pointer;
  background: rgba(0, 0, 0, 0.18);
}
.rooms li.selected {
  box-shadow: inset 0 0 0 1px var(--accent);
}
.rname {
  font-size: 0.9rem;
  color: var(--ink);
}
.meta {
  font-size: 0.75rem;
  color: var(--ink-muted);
}
.empty {
  margin: 0;
  font-size: 0.8rem;
  color: var(--ink-muted);
}
.cta {
  width: 100%;
  border: none;
  border-radius: 12px;
  padding: 0.85rem 1.1rem;
  font-size: 0.95rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  cursor: pointer;
  color: #0c1210;
  background: linear-gradient(135deg, var(--accent) 0%, var(--accent-dim) 100%);
}
.cta.secondary {
  color: var(--ink);
  background: rgba(232, 240, 236, 0.1);
  box-shadow: inset 0 0 0 1px var(--line);
}
.cta:disabled {
  opacity: 0.7;
  cursor: wait;
}
.linkish {
  border: none;
  background: transparent;
  color: var(--accent);
  font-size: 0.75rem;
  cursor: pointer;
  padding: 0;
}
</style>

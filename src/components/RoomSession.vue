<script setup lang="ts">
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ref } from "vue";
import type { RoomCredentials, RoomMember } from "../lib/roomApi";
import type { ConnKind, MemberNetInfo } from "../types/network";
import ConnBadge from "./ConnBadge.vue";

const props = defineProps<{
  session: RoomCredentials;
  members: RoomMember[];
  busy: boolean;
  netOf: (member: RoomMember) => MemberNetInfo;
}>();

const emit = defineEmits<{
  leave: [];
}>();

const copiedCode = ref(false);
const copiedIp = ref<string | null>(null);

async function copyCode() {
  try {
    await writeText(props.session.code);
    copiedCode.value = true;
    setTimeout(() => {
      copiedCode.value = false;
    }, 1500);
  } catch {
    copiedCode.value = false;
  }
}

async function copyIp(ip: string) {
  if (!ip) return;
  try {
    await writeText(ip);
    copiedIp.value = ip;
    setTimeout(() => {
      if (copiedIp.value === ip) copiedIp.value = null;
    }, 1200);
  } catch {
    copiedIp.value = null;
  }
}

function latencyLabel(net: MemberNetInfo): string {
  if (net.isSelf) return "—";
  if (net.kind === "pending") return "…";
  if (net.latencyMs == null) return "…";
  return `${net.latencyMs} ms`;
}

function peerKind(net: MemberNetInfo): ConnKind {
  if (
    net.kind === "self" ||
    net.kind === "pending"
  ) {
    return "unknown";
  }
  return net.kind;
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
          {{ copiedCode ? "已复制" : "复制邀请码" }}
        </button>
      </div>
      <p class="hint">把邀请码发给队友，对方输入后即可加入</p>
    </div>

    <div class="members">
      <header class="head">
        <h2>房间成员</h2>
        <span class="count">{{ members.length }}</span>
      </header>
      <ul v-if="members.length">
        <li v-for="m in members" :key="m.id" class="member">
          <div class="meta">
            <div class="name-row">
              <span class="dname">{{ m.displayName }}</span>
              <span v-if="netOf(m).isSelf" class="tag me">我</span>
              <span v-if="m.isHost" class="tag">网管</span>
            </div>
            <button
              v-if="netOf(m).virtualIp"
              type="button"
              class="ip"
              @click="copyIp(netOf(m).virtualIp!)"
            >
              {{ netOf(m).virtualIp }}
              <span class="hint-inline">
                {{ copiedIp === netOf(m).virtualIp ? "已复制" : "复制" }}
              </span>
            </button>
            <span v-else class="ip muted">连接中…</span>
          </div>
          <div class="side">
            <span v-if="netOf(m).isSelf" class="self-badge">本机</span>
            <span v-else-if="netOf(m).kind === 'pending'" class="pending">等待上线</span>
            <ConnBadge
              v-else
              :kind="peerKind(netOf(m))"
              :relay="netOf(m).relay"
            />
            <span class="rtt">{{ latencyLabel(netOf(m)) }}</span>
          </div>
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
}
.member {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 0.75rem;
  padding: 0.7rem 0;
  border-bottom: 1px solid var(--line);
}
.member:last-child {
  border-bottom: none;
}
.meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}
.name-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  flex-wrap: wrap;
}
.dname {
  font-size: 0.95rem;
  font-weight: 600;
}
.tag {
  font-size: 0.65rem;
  color: var(--accent);
}
.tag.me {
  color: var(--ink-muted);
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
  gap: 0.35rem;
}
.ip:hover {
  color: var(--accent);
}
.ip.muted {
  cursor: default;
}
.hint-inline {
  font-size: 0.65rem;
  opacity: 0.7;
  font-family: var(--font-body);
}
.side {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 0.3rem;
  flex-shrink: 0;
}
.self-badge,
.pending {
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--ink-muted);
}
.rtt {
  font-variant-numeric: tabular-nums;
  font-size: 0.75rem;
  color: var(--ink-muted);
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

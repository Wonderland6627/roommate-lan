import { computed, onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdatePhase =
  | "idle"
  | "checking"
  | "upToDate"
  | "available"
  | "downloading"
  | "restarting"
  | "error";

function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = n;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function useUpdater() {
  const currentVersion = ref("");
  const phase = ref<UpdatePhase>("idle");
  const error = ref("");
  const availableVersion = ref("");
  const notes = ref("");
  const downloaded = ref(0);
  const contentLength = ref(0);
  const pending = ref<Update | null>(null);

  const progressLabel = computed(() => {
    if (phase.value !== "downloading") return "";
    if (contentLength.value <= 0) {
      return `已下载 ${formatBytes(downloaded.value)}`;
    }
    const pct = Math.min(
      100,
      Math.round((downloaded.value / contentLength.value) * 100),
    );
    return `${pct}% · ${formatBytes(downloaded.value)} / ${formatBytes(contentLength.value)}`;
  });

  const statusText = computed(() => {
    switch (phase.value) {
      case "checking":
        return "正在检查更新…";
      case "upToDate":
        return "已是最新版本";
      case "available":
        return `发现新版本 v${availableVersion.value}`;
      case "downloading":
        return `正在下载更新… ${progressLabel.value}`;
      case "restarting":
        return "安装完成，即将重启…";
      case "error":
        return error.value || "检查更新失败";
      default:
        return "";
    }
  });

  async function loadVersion() {
    try {
      currentVersion.value = await getVersion();
    } catch {
      currentVersion.value = "";
    }
  }

  async function checkForUpdates(opts?: { silent?: boolean }) {
    const silent = opts?.silent === true;
    if (phase.value === "checking" || phase.value === "downloading") return;

    error.value = "";
    phase.value = "checking";
    pending.value = null;
    availableVersion.value = "";
    notes.value = "";

    try {
      const update = await check();
      if (!update) {
        phase.value = silent ? "idle" : "upToDate";
        return;
      }

      pending.value = update;
      availableVersion.value = update.version;
      notes.value = update.body ?? "";
      phase.value = "available";
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // Dev / unsigned builds and offline networks should not block the UI.
      if (silent) {
        phase.value = "idle";
        error.value = "";
        return;
      }
      error.value = msg || "检查更新失败";
      phase.value = "error";
    }
  }

  async function downloadAndInstall() {
    const update = pending.value;
    if (!update) return;
    if (phase.value === "downloading" || phase.value === "restarting") return;

    error.value = "";
    downloaded.value = 0;
    contentLength.value = 0;
    phase.value = "downloading";

    try {
      await update.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case "Started":
            contentLength.value = event.data.contentLength ?? 0;
            downloaded.value = 0;
            break;
          case "Progress":
            downloaded.value += event.data.chunkLength;
            break;
          case "Finished":
            break;
        }
      });
      phase.value = "restarting";
      await relaunch();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      error.value = msg || "下载或安装更新失败";
      phase.value = "error";
    }
  }

  onMounted(async () => {
    await loadVersion();
    await checkForUpdates({ silent: true });
  });

  return {
    currentVersion,
    phase,
    error,
    availableVersion,
    notes,
    progressLabel,
    statusText,
    checkForUpdates,
    downloadAndInstall,
  };
}

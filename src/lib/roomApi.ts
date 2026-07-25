export type RoomSummary = {
  id: string;
  name: string;
  memberCount: number;
  expiresAt: number;
  createdAt: number;
};

export type RoomMember = {
  id: string;
  displayName: string;
  isHost: boolean;
  joinedAt: number;
  nodeId?: string | null;
  virtualIp?: string | null;
  /** Public egress IP seen by room-api (HTTPS). */
  egressIp?: string | null;
  /** Geo label from room-api, e.g. "日本 · 東京都 · 东京". */
  geoLabel?: string | null;
};

export type RoomCredentials = {
  code: string;
  expiresAt: number;
  loginServer: string;
  authKey: string;
  memberToken: string;
  memberId?: string;
  isHost: boolean;
  room: RoomSummary;
};

export type TrafficReport = {
  relayBytes: number;
  p2pBytes: number;
};

const REQUEST_TIMEOUT_MS = 10_000;

export class ApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

/** Credential invalid or room already gone — stop retrying / leave room state. */
export function isFatalRoomError(err: unknown): boolean {
  return err instanceof ApiError && (err.status === 403 || err.status === 404);
}

export function isRateLimitedError(err: unknown): boolean {
  return err instanceof ApiError && err.status === 429;
}

function apiErrorMessage(status: number, body: string): string {
  try {
    const parsed = JSON.parse(body) as { detail?: unknown };
    if (typeof parsed.detail === "string") return parsed.detail;
  } catch {
    // ignore
  }
  if (body.trim()) {
    const trimmed = body.trim();
    // Avoid dumping HTML gateway pages into the UI.
    if (trimmed.startsWith("<")) return `请求失败 (${status})`;
    return trimmed.slice(0, 200);
  }
  return `请求失败 (${status})`;
}

async function request<T>(
  baseUrl: string,
  path: string,
  init?: RequestInit,
): Promise<T> {
  const url = `${baseUrl.replace(/\/$/, "")}${path}`;
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  let resp: Response;
  try {
    resp = await fetch(url, {
      ...init,
      signal: controller.signal,
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        ...(init?.headers ?? {}),
      },
    });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") {
      throw new Error("房间服务请求超时，请稍后重试");
    }
    throw new Error("无法连接房间服务，请检查网络或 Login Server 配置");
  } finally {
    window.clearTimeout(timer);
  }
  const text = await resp.text();
  if (!resp.ok) {
    throw new ApiError(resp.status, apiErrorMessage(resp.status, text));
  }
  if (!text) {
    return {} as T;
  }
  return JSON.parse(text) as T;
}

function withTraffic(
  body: Record<string, unknown>,
  traffic?: TrafficReport | null,
): Record<string, unknown> {
  if (!traffic) return body;
  return {
    ...body,
    relayBytes: Math.max(0, Math.floor(traffic.relayBytes)),
    p2pBytes: Math.max(0, Math.floor(traffic.p2pBytes)),
  };
}

export async function listRooms(baseUrl: string): Promise<RoomSummary[]> {
  const data = await request<{ rooms: RoomSummary[] }>(baseUrl, "/api/rooms");
  return data.rooms ?? [];
}

export async function createRoom(
  baseUrl: string,
  name: string,
  displayName: string,
): Promise<RoomCredentials> {
  return request<RoomCredentials>(baseUrl, "/api/rooms", {
    method: "POST",
    body: JSON.stringify({ name, displayName }),
  });
}

export async function joinRoom(
  baseUrl: string,
  code: string,
  displayName: string,
): Promise<RoomCredentials> {
  return request<RoomCredentials>(baseUrl, "/api/join", {
    method: "POST",
    body: JSON.stringify({ code, displayName }),
  });
}

export async function listMembers(
  baseUrl: string,
  roomId: string,
  memberToken: string,
): Promise<{ room: RoomSummary; members: RoomMember[] }> {
  return request(baseUrl, `/api/rooms/${encodeURIComponent(roomId)}/members`, {
    headers: {
      "X-Member-Token": memberToken,
    },
  });
}

export async function reportPresence(
  baseUrl: string,
  roomId: string,
  opts: {
    memberToken: string;
    nodeId: string;
    virtualIp: string;
    traffic?: TrafficReport | null;
  },
): Promise<void> {
  await request(baseUrl, `/api/rooms/${encodeURIComponent(roomId)}/presence`, {
    method: "POST",
    body: JSON.stringify(
      withTraffic(
        {
          memberToken: opts.memberToken,
          nodeId: opts.nodeId,
          virtualIp: opts.virtualIp,
        },
        opts.traffic,
      ),
    ),
  });
}

export async function leaveRoom(
  baseUrl: string,
  roomId: string,
  memberToken: string,
  traffic?: TrafficReport | null,
): Promise<void> {
  await request(baseUrl, `/api/rooms/${encodeURIComponent(roomId)}/leave`, {
    method: "POST",
    body: JSON.stringify(withTraffic({ memberToken }, traffic)),
  });
}

export async function dissolveRoom(
  baseUrl: string,
  roomId: string,
  memberToken: string,
  traffic?: TrafficReport | null,
): Promise<void> {
  await request(baseUrl, `/api/rooms/${encodeURIComponent(roomId)}/dissolve`, {
    method: "POST",
    body: JSON.stringify(withTraffic({ memberToken }, traffic)),
  });
}

/** Best-effort room cleanup that can outlive the window (close / crash path). */
export function leaveOrDissolveKeepalive(
  baseUrl: string,
  roomId: string,
  memberToken: string,
  isHost: boolean,
  traffic?: TrafficReport | null,
): void {
  const action = isHost ? "dissolve" : "leave";
  const url = `${baseUrl.replace(/\/$/, "")}/api/rooms/${encodeURIComponent(roomId)}/${action}`;
  try {
    void fetch(url, {
      method: "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
      },
      body: JSON.stringify(withTraffic({ memberToken }, traffic)),
      keepalive: true,
    });
  } catch {
    // ignore — process is exiting
  }
}

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

function apiErrorMessage(status: number, body: string): string {
  try {
    const parsed = JSON.parse(body) as { detail?: unknown };
    if (typeof parsed.detail === "string") return parsed.detail;
  } catch {
    // ignore
  }
  if (body.trim()) return body.trim().slice(0, 200);
  return `请求失败 (${status})`;
}

async function request<T>(
  baseUrl: string,
  path: string,
  init?: RequestInit,
): Promise<T> {
  const url = `${baseUrl.replace(/\/$/, "")}${path}`;
  let resp: Response;
  try {
    resp = await fetch(url, {
      ...init,
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        ...(init?.headers ?? {}),
      },
    });
  } catch {
    throw new Error("无法连接房间服务，请检查网络或 Login Server 配置");
  }
  const text = await resp.text();
  if (!resp.ok) {
    throw new Error(apiErrorMessage(resp.status, text));
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
): Promise<{ room: RoomSummary; members: RoomMember[] }> {
  return request(baseUrl, `/api/rooms/${encodeURIComponent(roomId)}/members`);
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

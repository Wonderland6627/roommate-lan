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
};

export type RoomCredentials = {
  code: string;
  expiresAt: number;
  loginServer: string;
  authKey: string;
  memberToken: string;
  isHost: boolean;
  room: RoomSummary;
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

export async function leaveRoom(
  baseUrl: string,
  roomId: string,
  memberToken: string,
): Promise<void> {
  await request(baseUrl, `/api/rooms/${encodeURIComponent(roomId)}/leave`, {
    method: "POST",
    body: JSON.stringify({ memberToken }),
  });
}

export async function dissolveRoom(
  baseUrl: string,
  roomId: string,
  memberToken: string,
): Promise<void> {
  await request(baseUrl, `/api/rooms/${encodeURIComponent(roomId)}/dissolve`, {
    method: "POST",
    body: JSON.stringify({ memberToken }),
  });
}

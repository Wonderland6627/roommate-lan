export type ConnKind =
  | "p2p"
  | "derpRelay"
  | "peerRelay"
  | "idle"
  | "offline"
  | "unknown";

export interface PeerView {
  id: string;
  hostname: string;
  ips: string[];
  online: boolean;
  active: boolean;
  conn: ConnKind;
  relay?: string | null;
  curAddr?: string | null;
  latencyMs?: number | null;
  /** Cumulative bytes sent to this peer (tailscale TxBytes). */
  txBytes?: number;
  /** Cumulative bytes received from this peer (tailscale RxBytes). */
  rxBytes?: number;
}

export interface NetworkStatusDto {
  backendState: string;
  selfId: string;
  selfIps: string[];
  selfHostname: string;
  peers: PeerView[];
}

export type ConnectionPhase =
  | "idle"
  | "connecting"
  | "connected"
  | "disconnecting"
  | "error";

export type MemberNetKind = ConnKind | "self" | "pending";

export interface MemberNetInfo {
  kind: MemberNetKind;
  relay?: string | null;
  latencyMs: number | null;
  virtualIp: string | null;
  isSelf: boolean;
}

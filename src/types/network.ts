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
}

export interface NetworkStatusDto {
  backendState: string;
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

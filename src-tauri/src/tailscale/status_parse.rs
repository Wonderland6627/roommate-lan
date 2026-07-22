use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConnKind {
    P2p,
    DerpRelay,
    PeerRelay,
    Idle,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerView {
    pub id: String,
    pub hostname: String,
    pub ips: Vec<String>,
    pub online: bool,
    pub active: bool,
    pub conn: ConnKind,
    pub relay: Option<String>,
    pub cur_addr: Option<String>,
    pub latency_ms: Option<u32>,
    /// Cumulative bytes sent to this peer (`tailscale status` TxBytes).
    #[serde(default)]
    pub tx_bytes: u64,
    /// Cumulative bytes received from this peer (`tailscale status` RxBytes).
    #[serde(default)]
    pub rx_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub backend_state: String,
    /// Absent when talking to an older RoommateNetworkService build.
    #[serde(default)]
    pub self_id: String,
    pub self_ips: Vec<String>,
    pub self_hostname: String,
    pub peers: Vec<PeerView>,
}

/// Minimal peer fields we care about from `tailscale status --json`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawPeer {
    #[serde(default, rename = "ID")]
    pub id: Option<String>,
    #[serde(default, rename = "HostName")]
    pub host_name: Option<String>,
    #[serde(default, rename = "DNSName")]
    pub dns_name: Option<String>,
    #[serde(default, rename = "TailscaleIPs")]
    pub tailscale_ips: Option<Vec<String>>,
    #[serde(default, rename = "CurAddr")]
    pub cur_addr: Option<String>,
    #[serde(default, rename = "Relay")]
    pub relay: Option<String>,
    #[serde(default, rename = "PeerRelay")]
    pub peer_relay: Option<String>,
    #[serde(default, rename = "Online")]
    pub online: Option<bool>,
    #[serde(default, rename = "Active")]
    pub active: Option<bool>,
    #[serde(default, rename = "TxBytes")]
    pub tx_bytes: Option<u64>,
    #[serde(default, rename = "RxBytes")]
    pub rx_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct StatusJson {
    #[serde(default, rename = "BackendState")]
    backend_state: Option<String>,
    #[serde(default, rename = "Self")]
    self_node: Option<RawPeer>,
    #[serde(default, rename = "Peer")]
    peer: Option<std::collections::HashMap<String, RawPeer>>,
}

pub fn classify(peer: &RawPeer) -> ConnKind {
    let online = peer.online.unwrap_or(false);
    if !online {
        return ConnKind::Offline;
    }

    if non_empty(&peer.cur_addr) {
        return ConnKind::P2p;
    }
    if non_empty(&peer.peer_relay) {
        return ConnKind::PeerRelay;
    }
    if non_empty(&peer.relay) {
        return ConnKind::DerpRelay;
    }

    if peer.active.unwrap_or(false) {
        ConnKind::Unknown
    } else {
        ConnKind::Idle
    }
}

fn non_empty(v: &Option<String>) -> bool {
    v.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
}

fn peer_to_view(key: &str, peer: &RawPeer) -> PeerView {
    let hostname = peer
        .host_name
        .clone()
        .or_else(|| {
            peer.dns_name
                .as_ref()
                .map(|d| d.trim_end_matches('.').to_string())
        })
        .unwrap_or_else(|| key.to_string());

    PeerView {
        id: peer.id.clone().unwrap_or_else(|| key.to_string()),
        hostname,
        ips: peer.tailscale_ips.clone().unwrap_or_default(),
        online: peer.online.unwrap_or(false),
        active: peer.active.unwrap_or(false),
        conn: classify(peer),
        relay: peer.relay.clone().filter(|s| !s.is_empty()),
        cur_addr: peer.cur_addr.clone().filter(|s| !s.is_empty()),
        latency_ms: None,
        tx_bytes: peer.tx_bytes.unwrap_or(0),
        rx_bytes: peer.rx_bytes.unwrap_or(0),
    }
}

pub fn parse_status_json(json: &str) -> Result<NetworkStatus, String> {
    let raw: StatusJson =
        serde_json::from_str(json).map_err(|e| format!("status JSON 解析失败: {e}"))?;

    let self_node = raw.self_node.unwrap_or_default();
    let mut peers: Vec<PeerView> = raw
        .peer
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| peer_to_view(&k, &v))
        .collect();

    peers.sort_by(|a, b| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()));

    Ok(NetworkStatus {
        backend_state: raw.backend_state.unwrap_or_else(|| "Unknown".into()),
        self_id: self_node.id.clone().unwrap_or_default(),
        self_ips: self_node.tailscale_ips.unwrap_or_default(),
        self_hostname: self_node
            .host_name
            .or(self_node.dns_name)
            .unwrap_or_else(|| "local".into()),
        peers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn classify_p2p() {
        let peer = RawPeer {
            online: Some(true),
            cur_addr: Some("1.2.3.4:41641".into()),
            relay: Some("txy".into()),
            ..Default::default()
        };
        assert_eq!(classify(&peer), ConnKind::P2p);
    }

    #[test]
    fn classify_derp() {
        let peer = RawPeer {
            online: Some(true),
            cur_addr: Some("".into()),
            relay: Some("txy".into()),
            ..Default::default()
        };
        assert_eq!(classify(&peer), ConnKind::DerpRelay);
    }

    #[test]
    fn classify_offline() {
        let peer = RawPeer {
            online: Some(false),
            relay: Some("txy".into()),
            ..Default::default()
        };
        assert_eq!(classify(&peer), ConnKind::Offline);
    }

    #[test]
    fn parse_fixture() {
        let json = r#"{
          "BackendState": "Running",
          "Self": {
            "ID": "n1",
            "HostName": "roommate-alice",
            "TailscaleIPs": ["100.64.0.1"],
            "Online": true
          },
          "Peer": {
            "n2": {
              "ID": "n2",
              "HostName": "roommate-bob",
              "TailscaleIPs": ["100.64.0.2"],
              "Online": true,
              "Active": true,
              "CurAddr": "",
              "Relay": "txy",
              "TxBytes": 1024,
              "RxBytes": 2048
            }
          }
        }"#;

        let status = parse_status_json(json).unwrap();
        assert_eq!(status.backend_state, "Running");
        assert_eq!(status.self_id, "n1");
        assert_eq!(status.self_ips, vec!["100.64.0.1"]);
        assert_eq!(status.peers.len(), 1);
        assert_eq!(status.peers[0].conn, ConnKind::DerpRelay);
        assert_eq!(status.peers[0].hostname, "roommate-bob");
        assert_eq!(status.peers[0].tx_bytes, 1024);
        assert_eq!(status.peers[0].rx_bytes, 2048);
    }
}

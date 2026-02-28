use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Network service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Addresses to listen on.
    pub listen_addresses: Vec<String>,
    /// External addresses (for NAT traversal).
    pub external_addresses: Vec<String>,
    /// Bootstrap nodes.
    pub bootnodes: Vec<String>,
    /// Maximum number of connected peers.
    pub max_peers: usize,
    /// Minimum number of peers to maintain.
    pub min_peers: usize,
    /// Enable mDNS local peer discovery.
    pub enable_mdns: bool,
    /// Enable Kademlia DHT.
    pub enable_kademlia: bool,
    /// Path to persistent node key.
    pub node_key_path: Option<PathBuf>,
    /// Request timeout in seconds.
    pub request_timeout_secs: u64,
    /// Connection timeout in seconds.
    pub connection_timeout_secs: u64,
    /// Max message size in bytes.
    pub max_message_size: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addresses: vec!["/ip4/0.0.0.0/tcp/30303".to_string()],
            external_addresses: vec![],
            bootnodes: vec![],
            max_peers: 50,
            min_peers: 5,
            enable_mdns: true,
            enable_kademlia: true,
            node_key_path: None,
            request_timeout_secs: 30,
            connection_timeout_secs: 10,
            max_message_size: 4 * 1024 * 1024, // 4 MB
        }
    }
}

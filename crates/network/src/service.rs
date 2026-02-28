use crate::config::NetworkConfig;
use crate::error::NetworkError;
use crate::messages::{NetworkMessage, PeerStatus};
use futures::StreamExt;
use libp2p::{
    gossipsub, identify, kad, noise, ping, tcp, yamux, Multiaddr, PeerId,
};
use libp2p::swarm::NetworkBehaviour;
use rustchain_consensus::FinalityVote;
use rustchain_core::{Block, BlockNumber, SignedTransaction};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// GossipSub topic names.
pub mod topics {
    pub const NEW_BLOCKS: &str = "/rustchain/blocks/1.0.0";
    pub const NEW_TRANSACTIONS: &str = "/rustchain/transactions/1.0.0";
    pub const FINALITY_VOTES: &str = "/rustchain/finality/1.0.0";
}

/// Events emitted by the network service.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    BlockReceived { block: Block, peer: PeerId },
    TransactionReceived { tx: SignedTransaction, peer: PeerId },
    VoteReceived { vote: FinalityVote, peer: PeerId },
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    SyncRequired { peer: PeerId, their_height: BlockNumber },
}

/// Commands to send to the network service.
#[derive(Debug)]
pub enum NetworkCommand {
    BroadcastBlock(Block),
    BroadcastTransaction(SignedTransaction),
    BroadcastVote(FinalityVote),
    RequestBlocks { peer: PeerId, start: BlockNumber, count: u32 },
    BanPeer(PeerId),
    Shutdown,
}

/// The main network service.
pub struct NetworkService {
    config: NetworkConfig,
    event_tx: mpsc::Sender<NetworkEvent>,
    command_rx: mpsc::Receiver<NetworkCommand>,
    local_peer_id: PeerId,
    connected_peers: HashMap<PeerId, PeerStatus>,
}

impl NetworkService {
    /// Create a new network service.
    pub fn new(
        config: NetworkConfig,
    ) -> Result<
        (
            Self,
            mpsc::Receiver<NetworkEvent>,
            mpsc::Sender<NetworkCommand>,
        ),
        NetworkError,
    > {
        let (event_tx, event_rx) = mpsc::channel(1024);
        let (command_tx, command_rx) = mpsc::channel(1024);

        // Generate a random peer ID for now
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());

        info!("Local peer ID: {}", local_peer_id);

        let service = Self {
            config,
            event_tx,
            command_rx,
            local_peer_id,
            connected_peers: HashMap::new(),
        };

        Ok((service, event_rx, command_tx))
    }

    /// Start the network service main loop.
    pub async fn start(mut self, shutdown: CancellationToken) -> Result<(), NetworkError> {
        info!("Network service starting on {:?}", self.config.listen_addresses);

        // Build the swarm
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| NetworkError::Transport(e.to_string()))?
            .with_behaviour(|key| {
                // GossipSub
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .max_transmit_size(self.config.max_message_size)
                    .heartbeat_interval(Duration::from_secs(1))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .build()
                    .map_err(|e| format!("gossipsub config error: {}", e))?;

                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .map_err(|e| format!("gossipsub error: {}", e))?;

                // Kademlia
                let store = kad::store::MemoryStore::new(key.public().to_peer_id());
                let kademlia = kad::Behaviour::new(key.public().to_peer_id(), store);

                // Identify
                let identify = identify::Behaviour::new(identify::Config::new(
                    "/rustchain/1.0.0".to_string(),
                    key.public(),
                ));

                // Ping
                let ping = ping::Behaviour::default();

                Ok(ChainNetworkBehaviour {
                    gossipsub,
                    kademlia,
                    identify,
                    ping,
                })
            })
            .map_err(|e| NetworkError::Transport(e.to_string()))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // Listen on configured addresses
        for addr_str in &self.config.listen_addresses {
            let addr: Multiaddr = addr_str
                .parse()
                .map_err(|e| NetworkError::Config(format!("invalid address {}: {}", addr_str, e)))?;
            swarm
                .listen_on(addr.clone())
                .map_err(|e| NetworkError::Transport(e.to_string()))?;
            info!("Listening on {}", addr);
        }

        // Subscribe to gossip topics
        let block_topic = gossipsub::IdentTopic::new(topics::NEW_BLOCKS);
        let tx_topic = gossipsub::IdentTopic::new(topics::NEW_TRANSACTIONS);
        let vote_topic = gossipsub::IdentTopic::new(topics::FINALITY_VOTES);

        swarm.behaviour_mut().gossipsub.subscribe(&block_topic)
            .map_err(|e| NetworkError::Protocol(format!("subscribe error: {}", e)))?;
        swarm.behaviour_mut().gossipsub.subscribe(&tx_topic)
            .map_err(|e| NetworkError::Protocol(format!("subscribe error: {}", e)))?;
        swarm.behaviour_mut().gossipsub.subscribe(&vote_topic)
            .map_err(|e| NetworkError::Protocol(format!("subscribe error: {}", e)))?;

        // Connect to bootnodes
        for bootnode_str in &self.config.bootnodes {
            if let Ok(addr) = bootnode_str.parse::<Multiaddr>() {
                info!("Dialing bootnode: {}", addr);
                let _ = swarm.dial(addr);
            }
        }

        info!("Network service started");

        // Main event loop
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("Network service shutting down");
                    break;
                }
                command = self.command_rx.recv() => {
                    match command {
                        Some(NetworkCommand::BroadcastBlock(block)) => {
                            if let Ok(data) = bincode::serialize(&NetworkMessage::NewBlock(block)) {
                                let _ = swarm.behaviour_mut().gossipsub.publish(block_topic.clone(), data);
                            }
                        }
                        Some(NetworkCommand::BroadcastTransaction(tx)) => {
                            if let Ok(data) = bincode::serialize(&NetworkMessage::NewTransaction(tx)) {
                                let _ = swarm.behaviour_mut().gossipsub.publish(tx_topic.clone(), data);
                            }
                        }
                        Some(NetworkCommand::BroadcastVote(vote)) => {
                            if let Ok(data) = bincode::serialize(&NetworkMessage::FinalityVote(vote)) {
                                let _ = swarm.behaviour_mut().gossipsub.publish(vote_topic.clone(), data);
                            }
                        }
                        Some(NetworkCommand::Shutdown) | None => break,
                        _ => {}
                    }
                }
                event = swarm.select_next_some() => {
                    use libp2p::swarm::SwarmEvent;
                    match event {
                        SwarmEvent::Behaviour(ChainNetworkBehaviourEvent::Gossipsub(
                            gossipsub::Event::Message { message, .. }
                        )) => {
                            if let Ok(msg) = bincode::deserialize::<NetworkMessage>(&message.data) {
                                let peer = message.source.unwrap_or(PeerId::random());
                                match msg {
                                    NetworkMessage::NewBlock(block) => {
                                        let _ = self.event_tx.send(NetworkEvent::BlockReceived { block, peer }).await;
                                    }
                                    NetworkMessage::NewTransaction(tx) => {
                                        let _ = self.event_tx.send(NetworkEvent::TransactionReceived { tx, peer }).await;
                                    }
                                    NetworkMessage::FinalityVote(vote) => {
                                        let _ = self.event_tx.send(NetworkEvent::VoteReceived { vote, peer }).await;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("Listening on {}/p2p/{}", address, self.local_peer_id);
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            info!("Connected to peer: {}", peer_id);
                            let _ = self.event_tx.send(NetworkEvent::PeerConnected(peer_id)).await;
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            info!("Disconnected from peer: {}", peer_id);
                            self.connected_peers.remove(&peer_id);
                            let _ = self.event_tx.send(NetworkEvent::PeerDisconnected(peer_id)).await;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn connected_peer_count(&self) -> usize {
        self.connected_peers.len()
    }
}

/// Combined network behaviour.
#[derive(NetworkBehaviour)]
struct ChainNetworkBehaviour {
    gossipsub: gossipsub::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

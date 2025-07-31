use libp2p::{
    identity,
    mdns::{Mdns, MdnsConfig},
    swarm::{Swarm, SwarmBuilder, SwarmEvent},
    PeerId, NetworkBehaviour,
};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use libp2p_swarm as libp2p_swarm;
use tracing::info;
use zion_core::blockchain::Blockchain;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ZionBehaviourEvent")]
pub struct ZionBehaviour {
    mdns: Mdns,
    // Aggiungi comportamento personalizzato per blockchain
    blockchain_sync: BlockchainSyncBehaviour,
}

#[derive(Debug)]
pub enum ZionBehaviourEvent {
    Mdns { peers: Vec<PeerId> },
    BlockchainSync { peer: PeerId, data: Vec<u8> },
}

pub struct BlockchainSyncBehaviour;

impl NetworkBehaviour for BlockchainSyncBehaviour {
    type ProtocolsHandler = libp2p_swarm::dummy::ConnectionHandler;
    type OutEvent = ZionBehaviourEvent;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        libp2p_swarm::dummy::ConnectionHandler
    }

    fn inject_event(&mut self, peer: PeerId, data: Vec<u8>) {
        tracing::info!("Received blockchain sync from {}: {} bytes", peer, data.len());
    }
}

pub async fn start_p2p(blockchain: Arc<Mutex<Blockchain>>) -> Result<Swarm<ZionBehaviour>, Box<dyn Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Peer ID: {}", local_peer_id);

    let transport = libp2p::tcp::async_io::Transport::new()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(libp2p::noise::Config::xx(&local_key)?)
        .multiplex(libp2p::yamux::Config::default())
        .boxed();

    let mdns = Mdns::new(MdnsConfig::default()).await?;
    let blockchain_sync = BlockchainSyncBehaviour;
    let behaviour = ZionBehaviour { mdns, blockchain_sync };

    let mut swarm = SwarmBuilder::with_async_std_executor(transport, behaviour, local_peer_id)
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    Ok(swarm)
}

use libp2p::{
    identity,
    mdns::{tokio::Behaviour as Mdns, Config as MdnsConfig, Event as MdnsEvent},
    noise,
    request_response::{self, json::Behaviour as JsonBehaviour, ProtocolSupport},
    tcp, yamux, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use libp2p_swarm_derive::NetworkBehaviour;
use std::error::Error;
use std::time::Duration;

use super::sync::{SyncRequest, SyncResponse};

/// Application protocol carried over libp2p request-response.
pub const SYNC_PROTOCOL: &str = "/blockrock/sync/1.0.0";

/// Peers that go quiet are kept around long enough to answer a sync request.
const IDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(60);

pub type SyncBehaviour = JsonBehaviour<SyncRequest, SyncResponse>;
pub type SyncEvent = request_response::Event<SyncRequest, SyncResponse>;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "CustomEvent")]
pub struct MyBehaviour {
    pub mdns: Mdns,
    /// Chain synchronisation: block announcements and chain requests.
    pub sync: SyncBehaviour,
}

#[derive(Debug)]
pub enum CustomEvent {
    Mdns(MdnsEvent),
    Sync(SyncEvent),
}

impl From<MdnsEvent> for CustomEvent {
    fn from(event: MdnsEvent) -> Self {
        CustomEvent::Mdns(event)
    }
}

impl From<SyncEvent> for CustomEvent {
    fn from(event: SyncEvent) -> Self {
        CustomEvent::Sync(event)
    }
}

impl MyBehaviour {
    pub fn new(local_peer_id: PeerId) -> Result<Self, Box<dyn Error>> {
        let mdns = Mdns::new(MdnsConfig::default(), local_peer_id)?;
        let sync = JsonBehaviour::new(
            [(
                StreamProtocol::new(SYNC_PROTOCOL),
                ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );
        Ok(MyBehaviour { mdns, sync })
    }
}

/// Builds a swarm that discovers peers over mDNS and speaks the sync protocol.
/// The caller owns the chain and drives the event loop.
pub async fn start_p2p_node() -> Result<Swarm<MyBehaviour>, Box<dyn Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {:?}", local_peer_id);

    let behaviour = MyBehaviour::new(local_peer_id)?;
    let mut swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_| Ok(behaviour))?
        .with_swarm_config(|config| config.with_idle_connection_timeout(IDLE_CONNECTION_TIMEOUT))
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    Ok(swarm)
}

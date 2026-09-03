use blockrock_core::blockchain::Blockchain;
use libp2p::futures::StreamExt;
use libp2p::mdns::Event as MdnsEvent;
use libp2p::swarm::SwarmEvent;
use libp2p::PeerId;
use rocket::tokio::sync::broadcast;
use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::{select, sync::mpsc, sync::Mutex};
use tracing::info;
use zion_core::{
    api::{
        grpc::start_grpc,
        rest::{SensorReading, TronService},
    },
    config::Config,
    identity::NodeIdentity,
    monitoring::{run_monitoring_loop, shared_metabolic_status, Hypothalamus, ProcfsVitalsSource},
    network::{
        p2p::{start_p2p_node, CustomEvent},
        service::handle_sync_event,
        sync::{BlockAnnouncement, SyncRequest},
    },
    server::{self, ServerContext},
    storage::ChainStore,
};

/// Blocks waiting to be pushed to the peers. Sealing is much faster than
/// broadcasting, so the queue absorbs bursts.
const ANNOUNCEMENT_QUEUE: usize = 64;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Inizializza logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Carica configurazione
    let config = Config::load()?;
    println!("Tron Address: {}", config.tron_address);

    // Identità dell'autorità: la chiave con cui questo nodo sigilla i blocchi
    let identity =
        NodeIdentity::load_or_create(config.authority_name.clone(), &config.authority_key_path)?;

    // Carica la catena da disco, o ne crea una nuova al primo avvio
    let store = ChainStore::new(config.chain_path.clone());
    let mut chain = store.load_or_create(&identity.name)?;
    chain.register_authority(&identity.name, identity.signing_key.verifying_key());
    chain
        .validate()
        .map_err(|e| format!("catena non valida su disco: {}", e))?;
    store.save(&chain)?;
    println!(
        "Chain: {} blocchi da {} (autorità: {})",
        chain.blocks.len(),
        store.path().display(),
        identity.name
    );
    let blockchain: Arc<Mutex<Blockchain>> = Arc::new(Mutex::new(chain));

    // Canale broadcast per i sensori
    let (sensor_tx, _sensor_rx) = broadcast::channel::<SensorReading>(100);
    let tron_service = TronService::new(config.trongrid_api_key, config.tron_address);

    // I blocchi sigillati dall'API finiscono qui e da qui ai peer
    let (block_tx, mut block_rx) = mpsc::channel::<BlockAnnouncement>(ANNOUNCEMENT_QUEUE);

    // Il loop decide soltanto: nessun provider cloud o Docker viene azionato.
    let metabolic_status = shared_metabolic_status();
    let metabolic_handle = tokio::spawn(run_monitoring_loop(
        ProcfsVitalsSource::default(),
        Hypothalamus::default(),
        Arc::clone(&metabolic_status),
        Duration::from_secs(15),
    ));

    // Avvia nodo P2P
    let mut swarm = start_p2p_node().await?;
    let mut connected: HashSet<PeerId> = HashSet::new();
    let mut greeted: HashSet<PeerId> = HashSet::new();

    // Configura Rocket
    let rocket = server::build(ServerContext {
        blockchain: Arc::clone(&blockchain),
        identity,
        store: store.clone(),
        sensor_tx: sensor_tx.clone(),
        metabolic_status,
        tron_service,
        block_announcer: block_tx,
    });

    // Avvia server gRPC
    let port = 50051;
    let grpc_handle = tokio::spawn(start_grpc(Arc::clone(&blockchain), port));

    // Avvia server Rocket
    let rocket_handle = tokio::spawn(rocket.launch());

    // Loop principale per gestire eventi P2P
    let mut rocket_handle = Some(rocket_handle);
    let mut grpc_handle = Some(grpc_handle);

    loop {
        select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(CustomEvent::Mdns(MdnsEvent::Discovered(peers))) => {
                        for (peer_id, addr) in peers {
                            info!("peer scoperto: {} su {}", peer_id, addr);
                            swarm.add_peer_address(peer_id, addr);
                            // Alla prima comparsa gli chiediamo la catena: se è
                            // più lunga della nostra la adottiamo.
                            if greeted.insert(peer_id) {
                                swarm.behaviour_mut().sync.send_request(&peer_id, SyncRequest::GetChain);
                            }
                        }
                    }
                    SwarmEvent::Behaviour(CustomEvent::Mdns(MdnsEvent::Expired(peers))) => {
                        for (peer_id, addr) in peers {
                            info!("peer scaduto: {} su {}", peer_id, addr);
                            greeted.remove(&peer_id);
                        }
                    }
                    SwarmEvent::Behaviour(CustomEvent::Sync(event)) => {
                        handle_sync_event(&mut swarm, &blockchain, Some(&store), event).await;
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        connected.insert(peer_id);
                    }
                    SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
                        if num_established == 0 {
                            connected.remove(&peer_id);
                        }
                    }
                    _ => {}
                }
            }
            Some(announcement) = block_rx.recv() => {
                for peer_id in &connected {
                    swarm
                        .behaviour_mut()
                        .sync
                        .send_request(peer_id, SyncRequest::NewBlock(announcement.clone()));
                }
            }
            result = async { rocket_handle.as_mut().map(|h| h).unwrap().await }, if rocket_handle.is_some() => {
                result??;
                println!("Rocket server terminated");
                rocket_handle = None;
            }
            result = async { grpc_handle.as_mut().map(|h| h).unwrap().await }, if grpc_handle.is_some() => {
                result??;
                println!("gRPC server terminated");
                grpc_handle = None;
            }
            else => break,
        }
    }

    metabolic_handle.abort();
    Ok(())
}

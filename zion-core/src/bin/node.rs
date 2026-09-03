use blockrock_core::blockchain::Blockchain;
use libp2p::futures::StreamExt;
use libp2p::mdns::Event as MdnsEvent;
use libp2p::swarm::SwarmEvent;
use rocket::tokio::sync::broadcast;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::{select, sync::Mutex};
use zion_core::{
    api::{
        grpc::start_grpc,
        rest::{SensorReading, TronService},
    },
    config::Config,
    identity::NodeIdentity,
    monitoring::{run_monitoring_loop, shared_metabolic_status, Hypothalamus, ProcfsVitalsSource},
    network::p2p::{start_p2p_node, CustomEvent},
    server::{self, ServerContext},
    storage::ChainStore,
};

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
    let identity = NodeIdentity::load_or_create(
        config.authority_name.clone(),
        &config.authority_key_path,
    )?;

    // Carica la catena da disco, o ne crea una nuova al primo avvio
    let store = ChainStore::new(config.chain_path.clone());
    let mut chain = store.load_or_create(&identity.name)?;
    chain.register_authority(&identity.name, identity.signing_key.verifying_key());
    chain.validate().map_err(|e| format!("catena non valida su disco: {}", e))?;
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

    // Il loop decide soltanto: nessun provider cloud o Docker viene azionato.
    let metabolic_status = shared_metabolic_status();
    let metabolic_handle = tokio::spawn(run_monitoring_loop(
        ProcfsVitalsSource::default(),
        Hypothalamus::default(),
        Arc::clone(&metabolic_status),
        Duration::from_secs(15),
    ));

    // Avvia nodo P2P
    let mut swarm = start_p2p_node(Arc::clone(&blockchain)).await?;

    // Configura Rocket
    let rocket = server::build(ServerContext {
        blockchain: Arc::clone(&blockchain),
        identity,
        store,
        sensor_tx: sensor_tx.clone(),
        metabolic_status,
        tron_service,
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
                            println!("Discovered peer: {} at {}", peer_id, addr);
                            swarm.dial(peer_id)?;
                        }
                    }
                    SwarmEvent::Behaviour(CustomEvent::Mdns(MdnsEvent::Expired(peers))) => {
                        for (peer_id, addr) in peers {
                            println!("Expired peer: {} at {}", peer_id, addr);
                        }
                    }
                    _ => {}
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

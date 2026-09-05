use blockrock_core::blockchain::Blockchain;
use libp2p::futures::StreamExt;
use libp2p::mdns::Event as MdnsEvent;
use libp2p::swarm::SwarmEvent;
use libp2p::PeerId;
use rocket::tokio::sync::broadcast;
use std::collections::HashSet;
use std::error::Error;
use std::future::pending;
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use tokio::{select, sync::mpsc, sync::oneshot, sync::Mutex};
use tracing::{error, info};
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


    // Avvia server gRPC. Il canale gli dice quando smettere di servire: senza
    // di esso il gRPC sopravviveva al resto del nodo.
    let port = 50051;
    let (grpc_shutdown_tx, grpc_shutdown_rx) = oneshot::channel::<()>();
    let grpc_handle = tokio::spawn(start_grpc(Arc::clone(&blockchain), port, async move {
        let _ = grpc_shutdown_rx.await;
    }));

    // Accendiamo Rocket prima di lanciarlo: `ignite` restituisce la maniglia
    // con cui fermarlo con grazia quando arriva il segnale.
    let rocket = rocket.ignite().await?;
    let rocket_shutdown = rocket.shutdown();
    let rocket_handle = tokio::spawn(rocket.launch());

    // Loop principale per gestire eventi P2P
    let mut rocket_handle = Some(rocket_handle);
    let mut grpc_handle = Some(grpc_handle);
    let mut grpc_shutdown_tx = Some(grpc_shutdown_tx);
    // Il futuro del segnale vive fuori dal loop: `select!` cancella i rami che
    // non si completano, e ricrearlo a ogni giro perderebbe un SIGTERM
    // arrivato nel frattempo.
    let mut shutdown = pin!(shutdown_signal());
    // Vero appena qualcosa — un segnale o la morte di un servizio — decide che
    // il nodo deve fermarsi. Da lì in poi si aspetta solo che i task escano.
    let mut stopping = false;
    let mut failure: Option<String> = None;

    loop {
        select! {
            _ = shutdown.as_mut(), if !stopping => {
                info!("segnale di arresto ricevuto: fermo i servizi");
                stopping = true;
            }
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
            result = async { rocket_handle.as_mut().unwrap().await }, if rocket_handle.is_some() => {
                rocket_handle = None;
                match result {
                    Ok(Ok(_)) => info!("server Rocket terminato"),
                    Ok(Err(e)) => record_failure(&mut failure, format!("server Rocket: {}", e)),
                    Err(e) => record_failure(&mut failure, format!("task Rocket: {}", e)),
                }
                // Senza l'API REST il nodo non offre più nulla: si scende tutti.
                stopping = true;
            }
            result = async { grpc_handle.as_mut().unwrap().await }, if grpc_handle.is_some() => {
                grpc_handle = None;
                match result {
                    Ok(Ok(())) => info!("server gRPC terminato"),
                    // Qui finisce anche la 50051 già occupata: prima uccideva il
                    // nodo lasciando Rocket in piedi a metà.
                    Ok(Err(e)) => record_failure(&mut failure, format!("server gRPC: {}", e)),
                    Err(e) => record_failure(&mut failure, format!("task gRPC: {}", e)),
                }
                stopping = true;
            }
        }

        if stopping {
            // Entrambe le maniglie sono idempotenti: `notify` lavora su un clone
            // e il oneshot si consuma al primo `take`.
            rocket_shutdown.clone().notify();
            if let Some(tx) = grpc_shutdown_tx.take() {
                let _ = tx.send(());
            }
        }

        // Si esce solo quando i due server hanno davvero chiuso, così il loop
        // P2P resta reattivo mentre drenano.
        if rocket_handle.is_none() && grpc_handle.is_none() {
            break;
        }
    }

    metabolic_handle.abort();

    if let Some(message) = failure {
        return Err(message.into());
    }
    info!("nodo arrestato");
    Ok(())
}

/// Tiene il primo errore: è quello che ha innescato l'arresto, gli altri sono
/// la sua eco.
fn record_failure(slot: &mut Option<String>, message: String) {
    error!("{}", message);
    slot.get_or_insert(message);
}

/// Si risolve a Ctrl-C o SIGTERM, i due modi in cui a questo nodo viene chiesto
/// di fermarsi. Se un handler non si registra restiamo in ascolto sull'altro
/// invece di fingere un arresto.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("impossibile ascoltare Ctrl-C: {}", e);
            pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(e) => {
                error!("impossibile ascoltare SIGTERM: {}", e);
                pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = pending::<()>();

    select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

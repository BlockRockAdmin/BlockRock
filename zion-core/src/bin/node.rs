use libp2p::futures::StreamExt;
use libp2p::mdns::Event as MdnsEvent;
use libp2p::swarm::SwarmEvent;
use rocket::routes;
use std::error::Error;
use std::sync::Arc;
use tokio::{sync::Mutex, select};
use zion_core::{
    api::{prometheus::init_metrics, grpc::start_grpc, rest::{get_blocks, get_balances, tron_balance, health, get_modules}},
    config::Config,
    network::p2p::{start_p2p_node, CustomEvent}
};
use blockrock_core::blockchain::Blockchain;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Inizializza logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Carica configurazione
    let config = Config::load()?;
    println!("TronGrid API Key loaded: {}", config.trongrid_api_key);
    println!("Tron Address: {}", config.tron_address);

    // Inizializza blockchain

	let blockchain = Arc::new(Mutex::new(Blockchain::new("default_authority".to_string())));
  
    // Avvia nodo P2P
    let mut swarm = start_p2p_node(Arc::clone(&blockchain)).await?;

    // Configura Rocket
    let rocket = rocket::build()
        .manage(Arc::clone(&blockchain))
        .mount("/", routes![get_blocks, get_balances, tron_balance, health, get_modules]);
    let rocket = init_metrics(rocket);

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

    Ok(())
}

//! Driving the sync protocol: what a node does when a peer talks to it.
//!
//! Kept in the library rather than in the node binary so the demo and the
//! integration tests exercise exactly the code the node runs.

use blockrock_core::blockchain::Blockchain;
use libp2p::request_response::{Event as SyncProtocolEvent, Message as SyncProtocolMessage};
use libp2p::Swarm;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::p2p::{MyBehaviour, SyncEvent};
use super::sync::{
    apply_announcement, apply_snapshot, snapshot_of, BlockOutcome, SyncRequest, SyncResponse,
};
use crate::storage::ChainStore;

/// Answers chain requests and applies what peers send us. Anything that
/// changes the chain is written to disk before we move on; `store` is `None`
/// for in-memory nodes such as tests and the demo.
pub async fn handle_sync_event(
    swarm: &mut Swarm<MyBehaviour>,
    blockchain: &Arc<Mutex<Blockchain>>,
    store: Option<&ChainStore>,
    event: SyncEvent,
) {
    match event {
        SyncProtocolEvent::Message { peer, message } => match message {
            SyncProtocolMessage::Request {
                request, channel, ..
            } => {
                let (response, ask_for_chain) = match request {
                    SyncRequest::GetChain => {
                        let chain = blockchain.lock().await;
                        (SyncResponse::Chain(snapshot_of(&chain)), false)
                    }
                    SyncRequest::NewBlock(announcement) => {
                        let index = announcement.block.index;
                        let mut chain = blockchain.lock().await;
                        match apply_announcement(&mut chain, announcement) {
                            Ok(BlockOutcome::Appended) => {
                                info!("blocco {} accettato da {}", index, peer);
                                persist(store, &chain);
                                (SyncResponse::Ack, false)
                            }
                            Ok(BlockOutcome::Known) => (SyncResponse::Ack, false),
                            // Non estende la nostra punta: solo la catena
                            // completa può dire chi dei due è indietro.
                            Ok(BlockOutcome::OutOfOrder) => (SyncResponse::Ack, true),
                            Err(failure) => {
                                warn!("blocco {} rifiutato da {}: {}", index, peer, failure);
                                (SyncResponse::Ack, false)
                            }
                        }
                    }
                };

                if swarm
                    .behaviour_mut()
                    .sync
                    .send_response(channel, response)
                    .is_err()
                {
                    warn!("risposta di sync non consegnata a {}", peer);
                }
                if ask_for_chain {
                    swarm
                        .behaviour_mut()
                        .sync
                        .send_request(&peer, SyncRequest::GetChain);
                }
            }
            SyncProtocolMessage::Response { response, .. } => {
                if let SyncResponse::Chain(snapshot) = response {
                    let mut chain = blockchain.lock().await;
                    match apply_snapshot(&mut chain, snapshot) {
                        Ok(true) => {
                            info!("catena adottata da {}: {} blocchi", peer, chain.blocks.len());
                            persist(store, &chain);
                        }
                        Ok(false) => {}
                        Err(failure) => warn!("catena di {} rifiutata: {}", peer, failure),
                    }
                }
            }
        },
        SyncProtocolEvent::OutboundFailure { peer, error, .. } => {
            warn!("richiesta di sync verso {} fallita: {}", peer, error);
        }
        SyncProtocolEvent::InboundFailure { peer, error, .. } => {
            warn!("richiesta di sync da {} fallita: {}", peer, error);
        }
        SyncProtocolEvent::ResponseSent { .. } => {}
    }
}

fn persist(store: Option<&ChainStore>, chain: &Blockchain) {
    let Some(store) = store else {
        return;
    };
    if let Err(failure) = store.save(chain) {
        error!(
            "catena non salvata su {}: {}",
            store.path().display(),
            failure
        );
    }
}

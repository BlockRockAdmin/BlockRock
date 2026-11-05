//! Best-effort propagation of validated pending transactions, separate from
//! the block sync protocol. An acknowledgement is not a committed block.

use blockrock_core::blockchain::{Blockchain, ChainError};
use blockrock_core::transaction::Transaction;
use ed25519_dalek::VerifyingKey;
use libp2p::request_response::{Event, Message, OutboundRequestId};
use libp2p::{PeerId, Swarm};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::warn;

use super::p2p::{MyBehaviour, TransactionEvent};

pub const GOSSIP_INTERVAL: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_secs(2);
const RECEIPT_TTL: Duration = Duration::from_secs(crate::ledger::BLOCK_TIME.as_secs() * 2);
const MAX_INFLIGHT_PER_PEER: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionAnnouncement {
    pub genesis_hash: String,
    pub transaction: Transaction,
    /// Only the sender's key, never an authority set or unrelated accounts.
    pub sender_key: VerifyingKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionResponse {
    Accepted,
    Known,
    Confirmed,
    Busy,
    Rejected,
}

/// Learning an unknown sender key is atomic with admission to the mempool.
/// A rejected packet cannot reserve a name or change the authority set.
pub fn apply_transaction(
    chain: &mut Blockchain,
    announcement: TransactionAnnouncement,
) -> Result<TransactionResponse, ChainError> {
    if chain.genesis_hash() != Some(announcement.genesis_hash.as_str()) {
        return Err(ChainError::GenesisMismatch);
    }
    let sender = announcement.transaction.sender.clone();
    let known_key = chain.public_key(&sender).copied();
    if known_key.is_none() && chain.authorities.contains(&sender) {
        // A transaction cannot establish the identity of a trusted validator.
        return Err(ChainError::UnknownSender(sender));
    }
    if known_key.is_some_and(|key| key != announcement.sender_key) {
        return Err(ChainError::InvalidSignature(announcement.transaction.id));
    }
    if known_key.is_none() {
        chain.add_public_key(&sender, announcement.sender_key);
    }
    let committed = chain
        .get_transaction(&announcement.transaction.id)
        .is_some();
    match chain.queue_transaction(announcement.transaction) {
        Ok(_) => Ok(TransactionResponse::Accepted),
        Err(error) => {
            if known_key.is_none() {
                chain.public_keys.remove(&sender);
            }
            if matches!(error, ChainError::DuplicateTransaction(_)) {
                Ok(if committed {
                    TransactionResponse::Confirmed
                } else {
                    TransactionResponse::Known
                })
            } else {
                Err(error)
            }
        }
    }
}

struct InFlight {
    peer: PeerId,
    id: String,
    sender: String,
}

/// Receipts are retained only while the transaction is pending locally and
/// the peer is connected. One in-flight nonce per sender avoids reordering.
#[derive(Default)]
pub struct TransactionGossip {
    known: HashMap<PeerId, HashMap<String, Instant>>,
    in_flight: HashMap<OutboundRequestId, InFlight>,
    retry_after: HashMap<PeerId, HashMap<String, Instant>>,
    chain_sync_after: HashMap<PeerId, Instant>,
}

impl TransactionGossip {
    /// Called on the timer, connection changes and protocol events. Re-reading
    /// the mempool also backfills newly connected peers, without an API queue
    /// that could silently discard an accepted transaction.
    pub fn drive(
        &mut self,
        swarm: &mut Swarm<MyBehaviour>,
        chain: &Blockchain,
        peers: &HashSet<PeerId>,
    ) {
        let mut pending: Vec<_> = chain.mempool.pending_transactions().collect();
        pending.sort_by(|a, b| a.sender.cmp(&b.sender).then(a.nonce.cmp(&b.nonce)));
        let ids: HashSet<_> = pending.iter().map(|tx| tx.id.as_str()).collect();
        self.known.retain(|peer, known| {
            known.retain(|id, until| ids.contains(id.as_str()) && *until > Instant::now());
            peers.contains(peer) && !known.is_empty()
        });
        self.in_flight.retain(|_, request| {
            peers.contains(&request.peer) && ids.contains(request.id.as_str())
        });
        self.retry_after.retain(|peer, deferred| {
            deferred.retain(|id, _| ids.contains(id.as_str()));
            peers.contains(peer) && !deferred.is_empty()
        });
        self.chain_sync_after.retain(|peer, _| peers.contains(peer));
        let Some(genesis) = chain.genesis_hash() else {
            return;
        };
        for peer in peers {
            let requests: Vec<_> = self
                .in_flight
                .values()
                .filter(|request| request.peer == *peer)
                .collect();
            let mut slots = MAX_INFLIGHT_PER_PEER.saturating_sub(requests.len());
            let mut senders: HashSet<_> = requests.iter().map(|r| r.sender.clone()).collect();
            for tx in &pending {
                if slots == 0 {
                    break;
                }
                if senders.contains(&tx.sender)
                    || self
                        .known
                        .get(peer)
                        .is_some_and(|known| known.contains_key(&tx.id))
                {
                    continue;
                }
                if self
                    .retry_after
                    .get(peer)
                    .and_then(|deferred| deferred.get(&tx.id))
                    .is_some_and(|deadline| *deadline > Instant::now())
                {
                    // Do not send a successor before this sender's prefix,
                    // but a refused transaction cannot starve other senders.
                    senders.insert(tx.sender.clone());
                    continue;
                }
                let Some(sender_key) = chain.public_key(&tx.sender) else {
                    continue;
                };
                let request_id = swarm.behaviour_mut().transactions.send_request(
                    peer,
                    TransactionAnnouncement {
                        genesis_hash: genesis.to_owned(),
                        transaction: (*tx).clone(),
                        sender_key: *sender_key,
                    },
                );
                self.in_flight.insert(
                    request_id,
                    InFlight {
                        peer: *peer,
                        id: tx.id.clone(),
                        sender: tx.sender.clone(),
                    },
                );
                senders.insert(tx.sender.clone());
                slots -= 1;
            }
        }
    }

    pub async fn handle_event(
        &mut self,
        swarm: &mut Swarm<MyBehaviour>,
        blockchain: &Arc<Mutex<Blockchain>>,
        event: TransactionEvent,
    ) {
        match event {
            Event::Message { peer, message } => match message {
                Message::Request {
                    request, channel, ..
                } => {
                    let result = apply_transaction(&mut *blockchain.lock().await, request);
                    let response = match result {
                        Ok(response) => response,
                        Err(ChainError::MempoolFull { .. }) => TransactionResponse::Busy,
                        Err(_) => TransactionResponse::Rejected,
                    };
                    if swarm
                        .behaviour_mut()
                        .transactions
                        .send_response(channel, response)
                        .is_err()
                    {
                        warn!(%peer, "ricevuta transazione non consegnata");
                    }
                }
                Message::Response {
                    request_id,
                    response,
                } => {
                    if let Some(request) = self.in_flight.remove(&request_id) {
                        if request.peer != peer {
                            return;
                        }
                        if matches!(
                            response,
                            TransactionResponse::Accepted
                                | TransactionResponse::Known
                                | TransactionResponse::Confirmed
                        ) {
                            self.known
                                .entry(peer)
                                .or_default()
                                .insert(request.id, Instant::now() + RECEIPT_TTL);
                            if response == TransactionResponse::Confirmed
                                && self
                                    .chain_sync_after
                                    .get(&peer)
                                    .is_none_or(|until| *until <= Instant::now())
                            {
                                // Repair a missed block notification using the
                                // existing fully validated snapshot protocol.
                                swarm
                                    .behaviour_mut()
                                    .sync
                                    .send_request(&peer, super::sync::SyncRequest::GetChain);
                                self.chain_sync_after
                                    .insert(peer, Instant::now() + RECEIPT_TTL);
                            }
                        } else {
                            // A peer may be full or behind our committed state.
                            // Neither response proves permanent invalidity.
                            self.retry_after
                                .entry(peer)
                                .or_default()
                                .insert(request.id, Instant::now() + RETRY_DELAY);
                        }
                    }
                }
            },
            Event::OutboundFailure {
                peer, request_id, ..
            } => {
                if let Some(request) = self.in_flight.remove(&request_id) {
                    self.retry_after
                        .entry(peer)
                        .or_default()
                        .insert(request.id, Instant::now() + RETRY_DELAY);
                }
            }
            Event::InboundFailure { peer, .. } => {
                warn!(%peer, "messaggio transazione non ricevuto");
            }
            Event::ResponseSent { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::p2p::start_p2p_node;
    use ed25519_dalek::SigningKey;
    use libp2p::request_response::OutboundFailure;
    use rand::rngs::OsRng;

    fn queued() -> (Blockchain, SigningKey) {
        let key = SigningKey::generate(&mut OsRng);
        let mut chain = Blockchain::new_single("validator".into());
        chain.add_public_key("Alice", key.verifying_key());
        for nonce in 0..2 {
            chain
                .queue_transaction(Transaction::new(
                    "Alice".into(),
                    "Bob".into(),
                    1,
                    nonce,
                    &key,
                ))
                .unwrap();
        }
        (chain, key)
    }

    #[tokio::test]
    async fn a_busy_peer_does_not_starve_other_senders_and_a_retry_preserves_nonce_order() {
        let (mut chain, key) = queued();
        let mut swarm = start_p2p_node().await.unwrap();
        let peer = PeerId::from(libp2p::identity::Keypair::generate_ed25519().public());
        let peers = HashSet::from([peer]);
        let mut gossip = TransactionGossip::default();
        gossip.drive(&mut swarm, &chain, &peers);
        assert_eq!(gossip.in_flight.len(), 1);
        let request_id = *gossip.in_flight.keys().next().unwrap();
        let first_id = gossip.in_flight[&request_id].id.clone();
        let blockchain = Arc::new(Mutex::new(chain.clone()));
        gossip
            .handle_event(
                &mut swarm,
                &blockchain,
                Event::Message {
                    peer,
                    message: Message::Response {
                        request_id,
                        response: TransactionResponse::Busy,
                    },
                },
            )
            .await;
        chain.add_public_key("Bob", key.verifying_key());
        chain
            .queue_transaction(Transaction::new("Bob".into(), "Bob".into(), 0, 0, &key))
            .unwrap();
        gossip.drive(&mut swarm, &chain, &peers);
        assert_eq!(gossip.in_flight.len(), 1);
        assert_eq!(gossip.in_flight.values().next().unwrap().sender, "Bob");
        gossip
            .retry_after
            .get_mut(&peer)
            .unwrap()
            .insert(first_id.clone(), Instant::now() - RETRY_DELAY);
        gossip.drive(&mut swarm, &chain, &peers);
        let request_id = *gossip
            .in_flight
            .iter()
            .find(|(_, r)| r.id == first_id)
            .unwrap()
            .0;
        gossip
            .handle_event(
                &mut swarm,
                &blockchain,
                Event::Message {
                    peer,
                    message: Message::Response {
                        request_id,
                        response: TransactionResponse::Accepted,
                    },
                },
            )
            .await;
        gossip.drive(&mut swarm, &chain, &peers);
        let next = gossip
            .in_flight
            .values()
            .find(|r| r.sender == "Alice")
            .unwrap();
        assert_ne!(next.id, first_id);
        assert_eq!(gossip.known[&peer].len(), 1);
    }

    #[tokio::test]
    async fn failed_delivery_can_retry_and_disconnected_peers_leave_no_receipt_cache() {
        let (chain, _) = queued();
        let mut swarm = start_p2p_node().await.unwrap();
        let peer = PeerId::from(libp2p::identity::Keypair::generate_ed25519().public());
        let peers = HashSet::from([peer]);
        let mut gossip = TransactionGossip::default();
        gossip.drive(&mut swarm, &chain, &peers);
        let request_id = *gossip.in_flight.keys().next().unwrap();
        let id = gossip.in_flight[&request_id].id.clone();
        gossip
            .handle_event(
                &mut swarm,
                &Arc::new(Mutex::new(chain.clone())),
                Event::OutboundFailure {
                    peer,
                    request_id,
                    error: OutboundFailure::Timeout,
                },
            )
            .await;
        assert!(gossip.in_flight.is_empty() && gossip.known.is_empty());
        gossip
            .retry_after
            .get_mut(&peer)
            .unwrap()
            .insert(id, Instant::now() - RETRY_DELAY);
        gossip.drive(&mut swarm, &chain, &peers);
        assert_eq!(gossip.in_flight.len(), 1);
        gossip.drive(&mut swarm, &chain, &HashSet::new());
        assert!(
            gossip.in_flight.is_empty() && gossip.known.is_empty() && gossip.retry_after.is_empty()
        );
    }

    #[tokio::test]
    async fn a_receipt_expires_when_the_confirming_block_never_arrived() {
        let (chain, _) = queued();
        let mut swarm = start_p2p_node().await.unwrap();
        let peer = PeerId::from(libp2p::identity::Keypair::generate_ed25519().public());
        let peers = HashSet::from([peer]);
        let mut gossip = TransactionGossip::default();
        gossip.drive(&mut swarm, &chain, &peers);
        let request_id = *gossip.in_flight.keys().next().unwrap();
        let id = gossip.in_flight[&request_id].id.clone();
        let blockchain = Arc::new(Mutex::new(chain.clone()));
        gossip
            .handle_event(
                &mut swarm,
                &blockchain,
                Event::Message {
                    peer,
                    message: Message::Response {
                        request_id,
                        response: TransactionResponse::Accepted,
                    },
                },
            )
            .await;
        gossip
            .known
            .get_mut(&peer)
            .unwrap()
            .insert(id.clone(), Instant::now() - RECEIPT_TTL);
        gossip.drive(&mut swarm, &chain, &peers);
        assert_eq!(gossip.in_flight.values().next().unwrap().id, id);
        let request_id = *gossip.in_flight.keys().next().unwrap();
        gossip
            .handle_event(
                &mut swarm,
                &blockchain,
                Event::Message {
                    peer,
                    message: Message::Response {
                        request_id,
                        response: TransactionResponse::Confirmed,
                    },
                },
            )
            .await;
        assert!(gossip.chain_sync_after.contains_key(&peer));
        gossip.drive(&mut swarm, &chain, &HashSet::new());
        assert!(gossip.chain_sync_after.is_empty());
    }
}

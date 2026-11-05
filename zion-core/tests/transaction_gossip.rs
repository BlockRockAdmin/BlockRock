use blockrock_core::blockchain::{Blockchain, ChainError, MINT_ACCOUNT};
use blockrock_core::transaction::{Transaction, MAX_PAYLOAD_BYTES};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{mpsc, Mutex};
use zion_core::identity::NodeIdentity;
use zion_core::ledger;
use zion_core::network::transactions::{
    apply_transaction, TransactionAnnouncement, TransactionResponse,
};
use zion_core::storage::ChainStore;

fn fixture() -> (Blockchain, SigningKey, SigningKey) {
    let authority = SigningKey::generate(&mut OsRng);
    let sender = SigningKey::generate(&mut OsRng);
    let mut chain = Blockchain::new_single("validator".into());
    chain.register_authority("validator", authority.verifying_key());
    (chain, authority, sender)
}

fn packet(
    chain: &Blockchain,
    sender: &SigningKey,
    amount: u64,
    nonce: u64,
) -> TransactionAnnouncement {
    TransactionAnnouncement {
        genesis_hash: chain.genesis_hash().unwrap().to_owned(),
        transaction: Transaction::new("Alice".into(), "Bob".into(), amount, nonce, sender),
        sender_key: sender.verifying_key(),
    }
}

#[test]
fn a_valid_packet_queues_without_committing_and_only_teaches_its_sender() {
    let (mut chain, _, sender) = fixture();
    let authorities = chain.authorities.clone();
    let message = packet(&chain, &sender, 10, 0);
    assert_eq!(
        apply_transaction(&mut chain, message),
        Ok(TransactionResponse::Accepted)
    );
    assert_eq!(chain.pending_count(), 1);
    assert_eq!(chain.blocks.len(), 1);
    assert_eq!(chain.balance_of("Alice"), 100);
    assert_eq!(chain.next_nonce("Alice"), 0);
    assert_eq!(chain.public_key("Alice"), Some(&sender.verifying_key()));
    assert_eq!(chain.public_keys.len(), 2);
    assert_eq!(chain.authorities, authorities);
}

#[test]
fn repeated_packets_are_known_before_and_after_the_block() {
    let (mut chain, authority, sender) = fixture();
    let message = packet(&chain, &sender, 10, 0);
    apply_transaction(&mut chain, message.clone()).unwrap();
    assert_eq!(
        apply_transaction(&mut chain, message.clone()),
        Ok(TransactionResponse::Known)
    );
    assert_eq!(chain.pending_count(), 1);
    chain.seal_mempool("validator", &authority).unwrap();
    assert_eq!(
        apply_transaction(&mut chain, message),
        Ok(TransactionResponse::Confirmed)
    );
    assert_eq!(chain.pending_count(), 0);
    assert_eq!(chain.balance_of("Alice"), 90);
}

#[test]
fn invalid_packets_do_not_learn_a_key_or_change_any_committed_state() {
    let (chain, _, sender) = fixture();
    let valid = packet(&chain, &sender, 10, 0);
    let mut variants = vec![
        packet(&chain, &sender, 101, 0),
        packet(&chain, &sender, 10, 1),
    ];
    let mut tampered = valid.clone();
    tampered.transaction.amount = 99;
    variants.push(tampered);
    let mut bad_signature = valid.clone();
    bad_signature.transaction.signature = None;
    variants.push(bad_signature);
    let mut foreign = valid.clone();
    foreign.genesis_hash = "another-network".into();
    variants.push(foreign);
    let mut oversized = valid.clone();
    oversized.transaction = Transaction::new_with_payload(
        "Alice".into(),
        "Bob".into(),
        0,
        0,
        Some("x".repeat(MAX_PAYLOAD_BYTES + 1)),
        &sender,
    );
    variants.push(oversized);
    let mut mint = valid;
    mint.transaction = Transaction::new(MINT_ACCOUNT.into(), "Bob".into(), 10, 0, &sender);
    variants.push(mint);
    for message in variants {
        let mut receiver = chain.clone();
        assert!(apply_transaction(&mut receiver, message).is_err());
        assert_eq!(receiver.public_keys, chain.public_keys);
        assert_eq!(receiver.authorities, chain.authorities);
        assert_eq!(receiver.balances, chain.balances);
        assert_eq!(receiver.nonces, chain.nonces);
        assert_eq!(receiver.blocks.len(), 1);
        assert_eq!(receiver.pending_count(), 0);
    }
}

#[test]
fn an_existing_sender_key_cannot_be_rebound_even_by_a_duplicate() {
    let (mut chain, _, sender) = fixture();
    let message = packet(&chain, &sender, 10, 0);
    apply_transaction(&mut chain, message.clone()).unwrap();
    let impostor = SigningKey::generate(&mut OsRng);
    let mut false_key = message;
    false_key.sender_key = impostor.verifying_key();
    assert!(matches!(
        apply_transaction(&mut chain, false_key),
        Err(ChainError::InvalidSignature(_))
    ));
    assert_eq!(chain.public_key("Alice"), Some(&sender.verifying_key()));
    assert_eq!(chain.pending_count(), 1);
}

#[test]
fn a_transaction_cannot_supply_a_missing_validator_key() {
    let mut chain = Blockchain::new_single("validator".into());
    let impostor = SigningKey::generate(&mut OsRng);
    let message = TransactionAnnouncement {
        genesis_hash: chain.genesis_hash().unwrap().into(),
        transaction: Transaction::new("validator".into(), "validator".into(), 0, 0, &impostor),
        sender_key: impostor.verifying_key(),
    };
    assert_eq!(
        apply_transaction(&mut chain, message),
        Err(ChainError::UnknownSender("validator".into()))
    );
    assert!(chain.public_key("validator").is_none());
    assert_eq!(chain.pending_count(), 0);
}

#[test]
fn a_full_pool_refuses_new_names_without_leaving_their_keys() {
    let (mut chain, _, sender) = fixture();
    chain.add_public_key("Alice", sender.verifying_key());
    for nonce in 0..1000 {
        let tx = Transaction::new_with_payload(
            "Alice".into(),
            "Alice".into(),
            0,
            nonce,
            Some("x".repeat(MAX_PAYLOAD_BYTES)),
            &sender,
        );
        if matches!(
            chain.queue_transaction(tx),
            Err(ChainError::MempoolFull { .. })
        ) {
            break;
        }
    }
    let before = chain.pending_count();
    let newcomer = SigningKey::generate(&mut OsRng);
    let message = TransactionAnnouncement {
        genesis_hash: chain.genesis_hash().unwrap().into(),
        transaction: Transaction::new_with_payload(
            "new-sensor".into(),
            "new-sensor".into(),
            0,
            0,
            Some("x".repeat(MAX_PAYLOAD_BYTES)),
            &newcomer,
        ),
        sender_key: newcomer.verifying_key(),
    };
    assert!(matches!(
        apply_transaction(&mut chain, message),
        Err(ChainError::MempoolFull { .. })
    ));
    assert!(chain.public_key("new-sensor").is_none());
    assert_eq!(chain.pending_count(), before);
}

#[tokio::test]
async fn a_follower_accepts_a_full_batch_without_trying_to_seal_it() {
    let (mut chain, _, sender) = fixture();
    chain.add_public_key("Alice", sender.verifying_key());
    let dir = TempDir::new().unwrap();
    let identity = NodeIdentity::load_or_create("follower".into(), dir.path().join("key")).unwrap();
    let store = ChainStore::new(dir.path().join("chain.json"));
    store.save(&chain).unwrap();
    let blockchain = Arc::new(Mutex::new(chain));
    let (announcer, mut announcements) = mpsc::channel(8);
    for nonce in 0..20 {
        let tx = Transaction::new_with_payload(
            "Alice".into(),
            "Alice".into(),
            0,
            nonce,
            Some("x".repeat(MAX_PAYLOAD_BYTES)),
            &sender,
        );
        ledger::submit(tx, &blockchain, &identity, &store, &announcer)
            .await
            .unwrap();
    }
    let chain = blockchain.lock().await;
    assert!(chain.is_worth_sealing());
    assert_eq!(chain.pending_count(), 20);
    assert_eq!(chain.blocks.len(), 1);
    assert!(announcements.try_recv().is_err());
}

use blockrock_core::block::Block;
use blockrock_core::blockchain::{Blockchain, ChainError};
use blockrock_core::transaction::Transaction;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

struct Fixture {
    chain: Blockchain,
    authority_key: SigningKey,
    alice_key: SigningKey,
}

fn fixture() -> Fixture {
    let authority_key = SigningKey::generate(&mut OsRng);
    let alice_key = SigningKey::generate(&mut OsRng);
    let mut chain = Blockchain::new("Node1".to_string());
    chain.register_authority("Node1", authority_key.verifying_key());
    chain.add_public_key("Alice", alice_key.verifying_key());
    Fixture {
        chain,
        authority_key,
        alice_key,
    }
}

#[test]
fn valid_transfer_moves_balance_and_nonce() {
    let Fixture {
        mut chain,
        authority_key,
        alice_key,
    } = fixture();

    let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 40, 0, &alice_key);
    chain.add_block(vec![tx], "Node1", &authority_key).unwrap();

    assert_eq!(chain.balance_of("Alice"), 60);
    assert_eq!(chain.balance_of("Bob"), 90);
    assert_eq!(chain.next_nonce("Alice"), 1);
    assert!(chain.validate_chain());
}

#[test]
fn double_spend_inside_one_block_is_rejected() {
    let Fixture {
        mut chain,
        authority_key,
        alice_key,
    } = fixture();

    // Alice holds 100: each transfer is affordable on its own, the pair is not.
    let first = Transaction::new("Alice".to_string(), "Bob".to_string(), 100, 0, &alice_key);
    let second = Transaction::new("Alice".to_string(), "Charlie".to_string(), 100, 1, &alice_key);

    let error = chain
        .add_block(vec![first, second], "Node1", &authority_key)
        .unwrap_err();

    assert_eq!(
        error,
        ChainError::InsufficientFunds {
            sender: "Alice".to_string(),
            balance: 0,
            amount: 100,
        }
    );
    // The rejected block left no trace.
    assert_eq!(chain.blocks.len(), 1);
    assert_eq!(chain.balance_of("Alice"), 100);
}

#[test]
fn replaying_a_signed_transaction_is_rejected() {
    let Fixture {
        mut chain,
        authority_key,
        alice_key,
    } = fixture();

    let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, 0, &alice_key);
    chain
        .add_block(vec![tx.clone()], "Node1", &authority_key)
        .unwrap();

    // Same bytes, resubmitted: the nonce has already been consumed.
    let error = chain
        .add_block(vec![tx], "Node1", &authority_key)
        .unwrap_err();
    assert!(matches!(error, ChainError::DuplicateTransaction(_)));

    // A fresh signature on a stale nonce is rejected too.
    let stale = Transaction::new("Alice".to_string(), "Charlie".to_string(), 10, 0, &alice_key);
    let error = chain
        .add_block(vec![stale], "Node1", &authority_key)
        .unwrap_err();
    assert_eq!(
        error,
        ChainError::InvalidNonce {
            sender: "Alice".to_string(),
            expected: 1,
            found: 0,
        }
    );
    assert_eq!(chain.balance_of("Alice"), 90);
}

#[test]
fn an_unauthorized_node_cannot_seal_a_block() {
    let Fixture {
        mut chain,
        alice_key,
        ..
    } = fixture();
    let intruder_key = SigningKey::generate(&mut OsRng);

    let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 1, 0, &alice_key);
    let error = chain
        .add_block(vec![tx.clone()], "Intruder", &intruder_key)
        .unwrap_err();
    assert_eq!(
        error,
        ChainError::UnauthorizedAuthority("Intruder".to_string())
    );

    // Nor can it borrow an authorized name it holds no key for.
    let error = chain.add_block(vec![tx], "Node1", &intruder_key).unwrap_err();
    assert_eq!(error, ChainError::AuthorityKeyMismatch("Node1".to_string()));
    assert_eq!(chain.blocks.len(), 1);
}

#[test]
fn a_forged_transaction_signature_is_rejected() {
    let Fixture {
        mut chain,
        authority_key,
        ..
    } = fixture();
    let forger_key = SigningKey::generate(&mut OsRng);

    // Signed by someone else, but claiming to be from Alice.
    let forged = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, 0, &forger_key);
    let error = chain
        .add_block(vec![forged], "Node1", &authority_key)
        .unwrap_err();
    assert!(matches!(error, ChainError::InvalidSignature(_)));
}

#[test]
fn tampering_with_a_stored_block_breaks_validation() {
    let Fixture {
        mut chain,
        authority_key,
        alice_key,
    } = fixture();

    let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, 0, &alice_key);
    chain.add_block(vec![tx], "Node1", &authority_key).unwrap();
    assert!(chain.validate_chain());

    chain.blocks[1].transactions[0].amount = 90;
    assert_eq!(chain.validate(), Err(ChainError::CorruptedHash(1)));
}

#[test]
fn an_unsealed_block_is_refused() {
    let Fixture {
        mut chain,
        alice_key,
        ..
    } = fixture();

    let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, 0, &alice_key);
    let unsealed = Block::new(1, vec![tx], chain.blocks[0].hash.clone(), "Node1".to_string());

    assert_eq!(
        chain.try_append_block(unsealed),
        Err(ChainError::UnsealedBlock(1))
    );
}

#[test]
fn a_peer_block_is_appended_when_it_is_valid() {
    let Fixture {
        mut chain,
        authority_key,
        alice_key,
    } = fixture();

    let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 25, 0, &alice_key);
    let block = Block::sealed(
        1,
        vec![tx],
        chain.blocks[0].hash.clone(),
        "Node1".to_string(),
        &authority_key,
    );

    chain.try_append_block(block).unwrap();
    assert_eq!(chain.balance_of("Alice"), 75);

    // A second block claiming the same height no longer links to the tip.
    let stale = Block::sealed(
        1,
        Vec::new(),
        chain.blocks[0].hash.clone(),
        "Node1".to_string(),
        &authority_key,
    );
    assert_eq!(
        chain.try_append_block(stale),
        Err(ChainError::WrongIndex {
            expected: 2,
            found: 1
        })
    );
}

#[test]
fn a_longer_valid_chain_is_adopted() {
    let Fixture {
        mut chain,
        authority_key,
        alice_key,
    } = fixture();

    let mut peer = Blockchain::new("Node1".to_string());
    peer.register_authority("Node1", authority_key.verifying_key());
    peer.add_public_key("Alice", alice_key.verifying_key());

    // Two nodes built from the same allocation share the same genesis.
    assert_eq!(chain.genesis_hash(), peer.genesis_hash());

    for nonce in 0..2 {
        let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, nonce, &alice_key);
        peer.add_block(vec![tx], "Node1", &authority_key).unwrap();
    }

    assert!(chain.try_adopt_chain(peer.get_blocks()).unwrap());
    assert_eq!(chain.blocks.len(), 3);
    assert_eq!(chain.balance_of("Alice"), 80);
    assert_eq!(chain.next_nonce("Alice"), 2);
    // The rebuilt index answers for transactions that arrived with the chain.
    let id = peer.blocks[1].transactions[0].id.clone();
    assert_eq!(chain.get_transaction(&id).unwrap().id, id);

    // Adopting the same chain again changes nothing: it is no longer longer.
    assert!(!chain.try_adopt_chain(peer.get_blocks()).unwrap());
}

#[test]
fn a_chain_from_another_network_is_refused() {
    let Fixture { mut chain, .. } = fixture();

    let mut stranger = Blockchain::new("OtherNode".to_string());
    let stranger_key = SigningKey::generate(&mut OsRng);
    stranger.register_authority("OtherNode", stranger_key.verifying_key());
    stranger
        .add_block(Vec::new(), "OtherNode", &stranger_key)
        .unwrap();
    stranger
        .add_block(Vec::new(), "OtherNode", &stranger_key)
        .unwrap();

    assert_eq!(
        chain.try_adopt_chain(stranger.get_blocks()),
        Err(ChainError::GenesisMismatch)
    );
    assert_eq!(chain.blocks.len(), 1);
}

#[test]
fn a_longer_chain_sealed_by_an_unknown_authority_is_refused() {
    let Fixture {
        mut chain,
        authority_key,
        ..
    } = fixture();

    // Same genesis (same authority name and allocation), but the extra blocks
    // are sealed by a key this node does not trust.
    let mut attacker = Blockchain::new("Node1".to_string());
    let attacker_key = SigningKey::generate(&mut OsRng);
    attacker.register_authority("Node1", attacker_key.verifying_key());
    attacker
        .add_block(Vec::new(), "Node1", &attacker_key)
        .unwrap();

    assert_eq!(chain.genesis_hash(), attacker.genesis_hash());
    assert_eq!(
        chain.try_adopt_chain(attacker.get_blocks()),
        Err(ChainError::InvalidSeal(1))
    );
    assert_eq!(chain.blocks.len(), 1);
    let _ = authority_key;
}

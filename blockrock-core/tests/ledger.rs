use blockrock_core::block::Block;
use blockrock_core::blockchain::{Blockchain, ChainError};
use blockrock_core::transaction::Transaction;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::collections::HashMap;

struct Fixture {
    chain: Blockchain,
    authority_key: SigningKey,
    alice_key: SigningKey,
}

fn fixture() -> Fixture {
    let authority_key = SigningKey::generate(&mut OsRng);
    let alice_key = SigningKey::generate(&mut OsRng);
    let mut chain = Blockchain::new_single("Node1".to_string());
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

    let mut peer = Blockchain::new_single("Node1".to_string());
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

    let mut stranger = Blockchain::new_single("OtherNode".to_string());
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
    let mut attacker = Blockchain::new_single("Node1".to_string());
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

#[test]
fn two_authorities_can_both_seal_blocks() {
    let alice = SigningKey::generate(&mut OsRng);
    let bob = SigningKey::generate(&mut OsRng);

    // Bootstrap a chain with two authorities at genesis.
    let mut initial = HashMap::new();
    initial.insert("Alice".to_string(), alice.verifying_key());
    initial.insert("Bob".to_string(), bob.verifying_key());

    let mut chain = Blockchain::new("Alice".to_string(), initial);
    // The local authority is always included; Bob must also be registered.
    chain.register_authority("Bob", bob.verifying_key());

    // Alice seals a block.
    chain
        .add_block(Vec::new(), "Alice", &alice)
        .unwrap();
    assert_eq!(chain.blocks.len(), 2);

    // Bob seals the next block.
    chain
        .add_block(Vec::new(), "Bob", &bob)
        .unwrap();
    assert_eq!(chain.blocks.len(), 3);

    // Both authorities are recognised.
    assert!(chain.authorities.contains("Alice"));
    assert!(chain.authorities.contains("Bob"));
    assert!(chain.validate_chain());
}

// --- Letture ancorate: transazioni che portano un dato firmato -------------

#[test]
fn a_reading_is_signed_together_with_the_rest_of_the_transaction() {
    let sensor = SigningKey::generate(&mut OsRng);
    let reading = Transaction::new_with_payload(
        "termometro".to_string(),
        "termometro".to_string(),
        0,
        0,
        Some("22.5".to_string()),
        &sensor,
    );

    assert_eq!(reading.payload.as_deref(), Some("22.5"));
    assert!(reading.verify(&sensor.verifying_key()));
}

#[test]
fn a_reading_altered_after_signing_is_rejected() {
    let sensor = SigningKey::generate(&mut OsRng);
    let mut reading = Transaction::new_with_payload(
        "termometro".to_string(),
        "termometro".to_string(),
        0,
        0,
        Some("22.5".to_string()),
        &sensor,
    );

    // Chi intercetta la lettura la ritocca lasciando intatta la firma.
    reading.payload = Some("35.0".to_string());

    assert!(!reading.has_valid_id(), "l'id non copre piu' il contenuto");
    assert!(!reading.verify(&sensor.verifying_key()));
}

#[test]
fn an_empty_reading_is_not_the_same_as_no_reading() {
    let none = Transaction::unsigned("a".to_string(), "b".to_string(), 1, 0);
    let empty = Transaction::unsigned_with_payload(
        "a".to_string(),
        "b".to_string(),
        1,
        0,
        Some(String::new()),
    );
    assert_ne!(
        none.signing_payload(),
        empty.signing_payload(),
        "None e Some(\"\") devono produrre byte diversi"
    );
    assert_ne!(none.id, empty.id);
}

#[test]
fn a_transaction_without_payload_signs_exactly_the_bytes_it_signed_before() {
    // Retrocompatibilita': i byte firmati da una transazione senza payload non
    // devono cambiare, altrimenti ogni catena gia' sigillata diventa invalida.
    let plain = Transaction::unsigned("Alice".to_string(), "Bob".to_string(), 30, 7);
    let bytes = plain.signing_payload();

    let domain = b"blockrock.transaction.v1".len();
    let expected = domain + (8 + "Alice".len()) + (8 + "Bob".len()) + 8 + 8;
    assert_eq!(
        bytes.len(),
        expected,
        "nessun byte in piu' deve essere aggiunto quando il payload e' assente"
    );
}

#[test]
fn an_anchored_reading_moves_no_value_but_stays_on_chain() {
    let Fixture {
        mut chain,
        authority_key,
        ..
    } = fixture();
    let sensor = SigningKey::generate(&mut OsRng);
    chain.add_public_key("termometro", sensor.verifying_key());

    let before = chain.balance_of("termometro");

    let reading = Transaction::new_with_payload(
        "termometro".to_string(),
        "termometro".to_string(),
        0,
        0,
        Some("22.5".to_string()),
        &sensor,
    );
    let id = reading.id.clone();
    chain.queue_transaction(reading).unwrap();
    chain
        .seal_mempool("Node1", &authority_key)
        .unwrap()
        .expect("il blocco deve essere sigillato");

    assert_eq!(
        before,
        chain.balance_of("termometro"),
        "una lettura non deve muovere valore"
    );

    let stored = chain
        .get_transaction(&id)
        .expect("la lettura deve essere in catena");
    assert_eq!(stored.payload.as_deref(), Some("22.5"));
    assert!(chain.validate().is_ok());
}

// --- Batching: piu' transazioni in un blocco solo ---------------------------

#[test]
fn many_transactions_share_a_single_block() {
    let Fixture {
        mut chain,
        authority_key,
        ..
    } = fixture();

    // Dieci mittenti diversi, ognuno con la sua chiave e il suo saldo.
    let mut senders = Vec::new();
    for i in 0..10 {
        let name = format!("sensore{}", i);
        let key = SigningKey::generate(&mut OsRng);
        chain.add_public_key(&name, key.verifying_key());
        senders.push((name, key));
    }

    for (name, key) in &senders {
        let reading = Transaction::new_with_payload(
            name.clone(),
            name.clone(),
            0,
            0,
            Some("22.5".to_string()),
            key,
        );
        chain.queue_transaction(reading).unwrap();
    }

    assert_eq!(chain.pending_count(), 10, "tutte in attesa, nessuna sigillata");
    assert!(
        !chain.is_worth_sealing(),
        "dieci letture non bastano a riempire un lotto"
    );

    let blocks_before = chain.blocks.len();
    chain
        .seal_mempool("Node1", &authority_key)
        .unwrap()
        .expect("il tick deve chiudere il blocco");

    assert_eq!(
        chain.blocks.len(),
        blocks_before + 1,
        "un blocco solo, non dieci"
    );
    assert_eq!(chain.blocks.last().unwrap().transactions.len(), 10);
    assert_eq!(chain.pending_count(), 0);
    assert_eq!(chain.pending_weight(), 0, "il peso segue il contenuto");
    assert!(chain.validate().is_ok());
}

#[test]
fn a_full_batch_asks_to_be_sealed() {
    let Fixture { mut chain, .. } = fixture();

    let mut queued = 0;
    while !chain.is_worth_sealing() {
        let name = format!("sensore{}", queued);
        let key = SigningKey::generate(&mut OsRng);
        chain.add_public_key(&name, key.verifying_key());
        // Payload al massimo consentito, per riempire in fretta.
        let reading = Transaction::new_with_payload(
            name.clone(),
            name,
            0,
            0,
            Some("x".repeat(blockrock_core::transaction::MAX_PAYLOAD_BYTES)),
            &key,
        );
        chain.queue_transaction(reading).unwrap();
        queued += 1;
        assert!(queued < 1000, "la soglia non viene mai raggiunta");
    }

    assert!(chain.pending_weight() >= blockrock_core::mempool::SEAL_THRESHOLD_BYTES);
}

// --- I due tetti che la soglia rende necessari ------------------------------

#[test]
fn a_payload_over_the_limit_never_reaches_the_mempool() {
    let Fixture { mut chain, .. } = fixture();
    let key = SigningKey::generate(&mut OsRng);
    chain.add_public_key("verboso", key.verifying_key());

    let too_big = Transaction::new_with_payload(
        "verboso".to_string(),
        "verboso".to_string(),
        0,
        0,
        Some("x".repeat(blockrock_core::transaction::MAX_PAYLOAD_BYTES + 1)),
        &key,
    );

    match chain.queue_transaction(too_big) {
        Err(ChainError::PayloadTooLarge { size, max, .. }) => {
            assert_eq!(size, max + 1);
        }
        other => panic!("atteso PayloadTooLarge, ottenuto {:?}", other),
    }
}

#[test]
fn a_block_from_a_peer_cannot_smuggle_an_oversized_payload() {
    // Il tetto e' una regola di consenso: un peer che ci manda un blocco gia'
    // sigillato non deve poter aggirare il controllo dell'API.
    let Fixture {
        mut chain,
        authority_key,
        ..
    } = fixture();
    let key = SigningKey::generate(&mut OsRng);
    chain.add_public_key("verboso", key.verifying_key());

    let too_big = Transaction::new_with_payload(
        "verboso".to_string(),
        "verboso".to_string(),
        0,
        0,
        Some("x".repeat(blockrock_core::transaction::MAX_PAYLOAD_BYTES + 1)),
        &key,
    );

    let result = chain.add_block(vec![too_big], "Node1", &authority_key);
    assert!(
        matches!(result, Err(ChainError::PayloadTooLarge { .. })),
        "la validazione della catena deve rifiutarlo, non solo l'API: {:?}",
        result
    );
}

#[test]
fn the_mempool_refuses_more_than_it_can_hold() {
    let Fixture { mut chain, .. } = fixture();
    let filler = "x".repeat(blockrock_core::transaction::MAX_PAYLOAD_BYTES);

    let mut queued = 0;
    let rejection = loop {
        let name = format!("sensore{}", queued);
        let key = SigningKey::generate(&mut OsRng);
        chain.add_public_key(&name, key.verifying_key());
        let reading = Transaction::new_with_payload(
            name.clone(),
            name,
            0,
            0,
            Some(filler.clone()),
            &key,
        );
        match chain.queue_transaction(reading) {
            Ok(_) => queued += 1,
            Err(error) => break error,
        }
        assert!(queued < 10_000, "il mempool non si riempie mai");
    };

    assert!(
        matches!(rejection, ChainError::MempoolFull { .. }),
        "atteso MempoolFull, ottenuto {:?}",
        rejection
    );
    assert!(
        chain.pending_weight() <= blockrock_core::mempool::MAX_MEMPOOL_BYTES,
        "il pool non deve superare il proprio tetto"
    );
}

#[test]
fn one_sender_can_queue_a_sequence_in_the_same_block() {
    let Fixture {
        mut chain,
        authority_key,
        alice_key,
    } = fixture();

    // Tre trasferimenti di fila, senza aspettare un blocco fra l'uno e
    // l'altro: i nonce proseguono contando anche la coda.
    for nonce in 0..3 {
        let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, nonce, &alice_key);
        chain
            .queue_transaction(tx)
            .unwrap_or_else(|e| panic!("nonce {} rifiutato: {}", nonce, e));
    }
    assert_eq!(chain.pending_count(), 3);

    chain.seal_mempool("Node1", &authority_key).unwrap().unwrap();

    assert_eq!(chain.blocks.last().unwrap().transactions.len(), 3);
    assert_eq!(chain.balance_of("Alice"), 70);
    assert_eq!(chain.balance_of("Bob"), 80);
    assert_eq!(chain.next_nonce("Alice"), 3);
    assert!(chain.validate().is_ok());
}

#[test]
fn a_queued_sequence_still_cannot_overspend() {
    let Fixture {
        mut chain,
        alice_key,
        ..
    } = fixture();

    // Alice ha 100. Due da 60 in coda sono 120: il secondo non deve passare,
    // anche se preso da solo sarebbe coperto dal saldo in catena.
    let first = Transaction::new("Alice".to_string(), "Bob".to_string(), 60, 0, &alice_key);
    chain.queue_transaction(first).unwrap();

    let second = Transaction::new("Alice".to_string(), "Bob".to_string(), 60, 1, &alice_key);
    assert!(
        matches!(
            chain.queue_transaction(second),
            Err(ChainError::InsufficientFunds { .. })
        ),
        "la coda dello stesso mittente deve contare nei fondi disponibili"
    );
}

#[test]
fn a_gap_in_the_sequence_is_still_refused() {
    let Fixture {
        mut chain,
        alice_key,
        ..
    } = fixture();

    let first = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, 0, &alice_key);
    chain.queue_transaction(first).unwrap();

    // Salta il nonce 1: la sequenza deve restare contigua.
    let skipped = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, 2, &alice_key);
    assert!(matches!(
        chain.queue_transaction(skipped),
        Err(ChainError::InvalidNonce { expected: 1, found: 2, .. })
    ));
}

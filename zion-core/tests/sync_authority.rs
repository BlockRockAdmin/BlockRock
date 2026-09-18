//! Un peer puo' farsi eleggere autorita' su un nodo che non lo conosce?

use blockrock_core::blockchain::Blockchain;
use blockrock_core::transaction::Transaction;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::collections::HashMap;
use zion_core::network::sync::{apply_announcement, BlockAnnouncement};

/// Una catena come quella di un nodo onesto: una sola autorita', Alice.
fn honest_chain() -> (Blockchain, SigningKey) {
    let alice = SigningKey::generate(&mut OsRng);
    let mut chain = Blockchain::new_single("alice".to_string());
    chain.register_authority("alice", alice.verifying_key());
    (chain, alice)
}

#[test]
fn a_rejected_block_leaves_no_trace() {
    let (mut chain, _alice) = honest_chain();
    let mallory = SigningKey::generate(&mut OsRng);

    // Mallory si costruisce una catena e ne annuncia un blocco, allegando la
    // propria chiave fra i conti.
    let mut theirs = Blockchain::new_single("mallory".to_string());
    theirs.register_authority("mallory", mallory.verifying_key());
    theirs.add_block(Vec::new(), "mallory", &mallory).unwrap();
    let block = theirs.blocks.last().unwrap().clone();

    let mut accounts = HashMap::new();
    accounts.insert("mallory".to_string(), mallory.verifying_key());

    let outcome = apply_announcement(&mut chain, BlockAnnouncement { block, accounts });
    assert!(outcome.is_err(), "il blocco deve essere rifiutato");

    // Il rifiuto non deve lasciare niente dietro di se'.
    assert!(
        chain.public_key("mallory").is_none(),
        "un blocco rifiutato non deve insegnarci la chiave di chi l'ha mandato"
    );
    assert!(
        !chain.authorities.contains("mallory"),
        "un blocco rifiutato non deve promuovere chi l'ha mandato ad autorita'"
    );
    assert!(
        !chain.clone().add_block(Vec::new(), "mallory", &mallory).is_ok(),
        "e mallory non deve poter sigillare sulla nostra catena"
    );
}

#[test]
fn an_announcement_cannot_squat_names_the_block_never_mentions() {
    // Un blocco valido, rilanciato da chiunque, non deve poter occupare nomi
    // che nel blocco non compaiono.
    let (mut chain, alice) = honest_chain();
    let mut author = chain.clone();
    author.add_block(Vec::new(), "alice", &alice).unwrap();
    let block = author.blocks.last().unwrap().clone();

    let intruder = SigningKey::generate(&mut OsRng);
    let mut accounts = HashMap::new();
    accounts.insert("termometro".to_string(), intruder.verifying_key());

    apply_announcement(&mut chain, BlockAnnouncement { block, accounts })
        .expect("il blocco e' valido e va accettato");

    assert!(
        chain.public_key("termometro").is_none(),
        "il blocco non nomina 'termometro': il nome deve restare libero"
    );
}

#[test]
fn the_sender_key_of_a_valid_block_is_still_learned() {
    // La stretta non deve rompere il caso legittimo: le chiavi dei mittenti
    // delle transazioni contenute nel blocco servono davvero.
    let (mut chain, alice) = honest_chain();
    let sensor = SigningKey::generate(&mut OsRng);

    let mut author = chain.clone();
    author.add_public_key("termometro", sensor.verifying_key());
    let reading = Transaction::new_with_payload(
        "termometro".to_string(),
        "termometro".to_string(),
        0,
        0,
        Some("22.5".to_string()),
        &sensor,
    );
    author.add_block(vec![reading], "alice", &alice).unwrap();
    let block = author.blocks.last().unwrap().clone();

    let mut accounts = HashMap::new();
    accounts.insert("termometro".to_string(), sensor.verifying_key());

    apply_announcement(&mut chain, BlockAnnouncement { block, accounts })
        .expect("un blocco valido con una lettura dentro va accettato");

    assert!(
        chain.public_key("termometro").is_some(),
        "la chiave del mittente serve a verificare la transazione"
    );
}

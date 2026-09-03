use blockrock_core::blockchain::Blockchain;
use blockrock_core::transaction::Transaction;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use tempfile::tempdir;

#[test]
fn save_and_load_chain() {
    let authority_key = SigningKey::generate(&mut OsRng);
    let alice_key = SigningKey::generate(&mut OsRng);

    let mut chain = Blockchain::new("Node1".to_string());
    chain.register_authority("Node1", authority_key.verifying_key());
    chain.add_public_key("Alice", alice_key.verifying_key());

    let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 5, 0, &alice_key);
    let transaction_id = tx.id.clone();
    chain
        .add_block(vec![tx], "Node1", &authority_key)
        .expect("the authority can seal a valid block");
    assert!(chain.validate_chain());
    assert_eq!(
        chain.get_transaction(&transaction_id).unwrap().id,
        transaction_id
    );

    let dir = tempdir().unwrap();
    let path = dir.path().join("nested").join("chain.json");
    chain.save_to_file(&path).unwrap();

    let loaded = Blockchain::load_from_file(&path).unwrap();
    assert!(loaded.validate_chain());
    assert_eq!(loaded.blocks.len(), chain.blocks.len());
    assert_eq!(loaded.balances, chain.balances);
    assert_eq!(loaded.nonces, chain.nonces);
    assert_eq!(
        loaded.get_transaction(&transaction_id).unwrap().id,
        transaction_id
    );
}

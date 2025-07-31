use blockrock_core::blockchain::Blockchain;
use blockrock_core::transaction::Transaction;
use ed25519_dalek::SigningKey;
use tempfile::tempdir;

#[test]
fn save_and_load_chain() {
    let mut chain = Blockchain::new("Node1".to_string());

    let key = SigningKey::generate(&mut rand::thread_rng());
    chain.add_public_key("Alice", key.verifying_key());

    let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 5.0, &key);
    assert!(chain.add_block(vec![tx], "Node1".to_string()));
    assert!(chain.validate_chain());

    let dir = tempdir().unwrap();
    let path = dir.path().join("chain.json");
    chain.save_to_file(&path).unwrap();

    let loaded = Blockchain::load_from_file(&path).unwrap();
    assert!(loaded.validate_chain());
    assert_eq!(loaded.blocks.len(), chain.blocks.len());
}

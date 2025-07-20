use std::collections::HashMap;
use super::block::Block;
use super::transaction::Transaction;
use ed25519_dalek::{VerifyingKey, SigningKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Blockchain {
    pub blocks: Vec<Block>,
    pub authority: String,
    pub balances: HashMap<String, f64>,
    pub public_keys: HashMap<String, VerifyingKey>,
}

impl Blockchain {
    pub fn new(authority: String) -> Self {
        let mut blockchain = Blockchain {
            blocks: Vec::new(),
            authority: authority.clone(),
            balances: HashMap::new(),
            public_keys: HashMap::new(),
        };
        blockchain.balances.insert("System".to_string(), 1000.0);
        blockchain.balances.insert("Alice".to_string(), 100.0);
        blockchain.balances.insert("Bob".to_string(), 50.0);
        blockchain.balances.insert("Charlie".to_string(), 30.0);
        blockchain.balances.insert("Node1".to_string(), 50.0);
        blockchain.balances.insert("Node2".to_string(), 0.0);

        // Genera una chiave temporanea per la transazione di genesi
        let temp_key = SigningKey::generate(&mut rand::thread_rng());
        let genesis_transaction = Transaction::new(
            "System".to_string(),
            "Genesis".to_string(),
            0.0,
            &temp_key,
        );
        let genesis_block = Block::new(
            0,
            vec![genesis_transaction],
            "0".to_string(),
            blockchain.authority.clone(),
        );
        blockchain.blocks.push(genesis_block);
        blockchain
    }

    pub fn add_block(&mut self, transactions: Vec<Transaction>, authority: String) -> bool {
        let previous_hash = self.blocks.last().map(|block| block.hash.clone()).unwrap_or("0".to_string());
        let new_block = Block::new(
            self.blocks.len() as u32,
            transactions.clone(),
            previous_hash,
            authority,
        );
        for transaction in &transactions {
            if transaction.sender != "System" {
                if let Some(sender_balance) = self.balances.get_mut(&transaction.sender) {
                    *sender_balance -= transaction.amount;
                }
            }
            if let Some(receiver_balance) = self.balances.get_mut(&transaction.receiver) {
                *receiver_balance += transaction.amount;
            } else {
                self.balances.insert(transaction.receiver.clone(), transaction.amount);
            }
        }
        self.blocks.push(new_block);
        true
    }

    pub fn get_blocks(&self) -> Vec<Block> {
        self.blocks.clone()
    }

    pub fn get_balances(&self) -> Vec<(String, f64)> {
        let mut balances: Vec<(String, f64)> = self.balances.iter().map(|(k, v)| (k.clone(), *v)).collect();
        balances.sort_by(|a, b| a.0.cmp(&b.0));
        balances
    }

    pub fn add_public_key(&mut self, name: &str, key: VerifyingKey) {
        self.public_keys.insert(name.to_string(), key);
    }

    pub fn get_transaction(&self, id: &str) -> Option<Transaction> {
        for block in &self.blocks {
            for tx in &block.transactions {
                if tx.id == id {
                    return Some(tx.clone());
                }
            }
        }
        None
    }
}

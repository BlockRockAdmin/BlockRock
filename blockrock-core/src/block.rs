use blake2::{Blake2b512, Digest};
use hex;
use serde::{Deserialize, Serialize};

use crate::transaction::Transaction;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub index: u32,
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub previous_hash: String,
    pub hash: String,
    pub authority: String,
}

impl Block {
    pub fn new(
        index: u32,
        transactions: Vec<Transaction>,
        previous_hash: String,
        authority: String,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut block = Block {
            index,
            timestamp,
            transactions,
            previous_hash,
            hash: String::new(),
            authority,
        };
        block.hash = block.calculate_hash();
        block
    }

    pub fn calculate_hash(&self) -> String {
        let mut hasher = Blake2b512::new();
        hasher.update(self.index.to_le_bytes());
        hasher.update(self.timestamp.to_le_bytes());
        for tx in &self.transactions {
            hasher.update(tx.id.as_bytes()); // Aggiunto id
            hasher.update(tx.sender.as_bytes());
            hasher.update(tx.receiver.as_bytes());
            hasher.update(tx.amount.to_le_bytes());
            if let Some(signature) = &tx.signature {
                hasher.update(signature.to_bytes());
            }
        }
        hasher.update(self.previous_hash.as_bytes());
        hasher.update(self.authority.as_bytes());
        let result = hasher.finalize();
        let hash_slice = &result[..32];
        hex::encode(hash_slice)
    }

    /// Verifica che l'hash memorizzato nel blocco sia corretto
    pub fn is_hash_valid(&self) -> bool {
        self.calculate_hash() == self.hash
    }
}

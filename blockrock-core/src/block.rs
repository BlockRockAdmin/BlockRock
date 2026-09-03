use blake2::{Blake2b512, Digest};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hex;
use serde::{Deserialize, Serialize};

use crate::transaction::Transaction;

const BLOCK_DOMAIN: &[u8] = b"blockrock.block.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub index: u32,
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub previous_hash: String,
    pub hash: String,
    /// Name of the authority that sealed the block.
    pub authority: String,
    /// Proof that `authority` produced this block. Only genesis is unsigned.
    #[serde(default)]
    pub authority_signature: Option<Signature>,
}

impl Block {
    /// Creates an unsealed block. Use [`Block::sealed`] for anything but genesis:
    /// an unsealed block is rejected by chain validation.
    pub fn new(
        index: u32,
        transactions: Vec<Transaction>,
        previous_hash: String,
        authority: String,
    ) -> Self {
        Self::with_timestamp(index, transactions, previous_hash, authority, now())
    }

    /// Same as [`Block::new`] with an explicit timestamp, so the genesis block
    /// is byte-identical on every node and chains stay comparable.
    pub fn with_timestamp(
        index: u32,
        transactions: Vec<Transaction>,
        previous_hash: String,
        authority: String,
        timestamp: u64,
    ) -> Self {
        let mut block = Block {
            index,
            timestamp,
            transactions,
            previous_hash,
            hash: String::new(),
            authority,
            authority_signature: None,
        };
        block.hash = block.calculate_hash();
        block
    }

    pub fn sealed(
        index: u32,
        transactions: Vec<Transaction>,
        previous_hash: String,
        authority: String,
        signing_key: &SigningKey,
    ) -> Self {
        let mut block = Self::new(index, transactions, previous_hash, authority);
        block.seal(signing_key);
        block
    }

    pub fn seal(&mut self, signing_key: &SigningKey) {
        self.authority_signature = Some(signing_key.sign(&self.signing_payload()));
    }

    /// The seal covers the block hash, which already commits to every field.
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(BLOCK_DOMAIN.len() + self.hash.len());
        payload.extend_from_slice(BLOCK_DOMAIN);
        payload.extend_from_slice(self.hash.as_bytes());
        payload
    }

    pub fn verify_seal(&self, verifying_key: &VerifyingKey) -> bool {
        match &self.authority_signature {
            Some(signature) => verifying_key
                .verify(&self.signing_payload(), signature)
                .is_ok(),
            None => false,
        }
    }

    /// Note: the seal is deliberately excluded, otherwise sealing would change
    /// the hash the seal commits to.
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Blake2b512::new();
        hasher.update(self.index.to_le_bytes());
        hasher.update(self.timestamp.to_le_bytes());
        for tx in &self.transactions {
            hasher.update(tx.id.as_bytes());
            hasher.update(tx.sender.as_bytes());
            hasher.update(tx.receiver.as_bytes());
            hasher.update(tx.amount.to_le_bytes());
            hasher.update(tx.nonce.to_le_bytes());
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

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

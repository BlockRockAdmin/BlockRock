use serde::{Serialize, Deserialize};
use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Verifier, Signature};
use sha2::{Sha256, Digest};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String, // Aggiunto
    pub sender: String,
    pub receiver: String,
    pub amount: f64,
    pub signature: Option<Signature>,
}

impl Transaction {
    pub fn new(sender: String, receiver: String, amount: f64, signing_key: &SigningKey) -> Self {
        let id = Self::generate_id(&sender, &receiver, amount);
        let mut transaction = Transaction {
            id,
            sender,
            receiver,
            amount,
            signature: None,
        };
        let signature = transaction.sign(signing_key);
        transaction.signature = Some(signature);
        transaction
    }

    pub fn sign(&self, signing_key: &SigningKey) -> Signature {
        let message = self.to_bytes();
        signing_key.sign(&message)
    }

    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        if let Some(signature) = &self.signature {
            let message = self.to_bytes();
            verifying_key.verify(&message, signature).is_ok()
        } else {
            false
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(self.id.as_bytes()); // Includi id nel hash
        hasher.update(self.sender.as_bytes());
        hasher.update(self.receiver.as_bytes());
        hasher.update(self.amount.to_le_bytes());
        hasher.finalize().to_vec()
    }

    fn generate_id(sender: &str, receiver: &str, amount: f64) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let input = format!("{}{}{}{}", sender, receiver, amount, timestamp);
        let mut hasher = Sha256::new();
        hasher.update(input);
        hex::encode(hasher.finalize())
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Transaction: {} -> {} ({}, id: {})", self.sender, self.receiver, self.amount, self.id)
    }
}

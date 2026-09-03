use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Amounts are integers in the smallest indivisible unit: a ledger must never
/// accumulate floating point rounding errors.
pub type Amount = u64;

const TRANSACTION_DOMAIN: &[u8] = b"blockrock.transaction.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    /// Hash of the signed payload: derived from the content, never random.
    pub id: String,
    pub sender: String,
    pub receiver: String,
    pub amount: Amount,
    /// Per-sender sequence number, starting at 0. Replaying a signed
    /// transaction fails because the sender has already consumed that nonce.
    pub nonce: u64,
    pub signature: Option<Signature>,
}

/// Canonical bytes a sender signs. Every variable-length field is
/// length-prefixed, so ("ab", "c") and ("a", "bc") cannot collide.
fn signing_payload(sender: &str, receiver: &str, amount: Amount, nonce: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(TRANSACTION_DOMAIN.len() + sender.len() + receiver.len() + 32);
    payload.extend_from_slice(TRANSACTION_DOMAIN);
    for field in [sender.as_bytes(), receiver.as_bytes()] {
        payload.extend_from_slice(&(field.len() as u64).to_le_bytes());
        payload.extend_from_slice(field);
    }
    payload.extend_from_slice(&amount.to_le_bytes());
    payload.extend_from_slice(&nonce.to_le_bytes());
    payload
}

impl Transaction {
    /// Builds and signs a transaction locally.
    pub fn new(
        sender: String,
        receiver: String,
        amount: Amount,
        nonce: u64,
        signing_key: &SigningKey,
    ) -> Self {
        let mut transaction = Self::unsigned(sender, receiver, amount, nonce);
        transaction.signature = Some(signing_key.sign(&transaction.signing_payload()));
        transaction
    }

    /// Rebuilds a transaction a client signed offline: the id is recomputed
    /// here, so a client cannot choose it.
    pub fn signed(
        sender: String,
        receiver: String,
        amount: Amount,
        nonce: u64,
        signature: Signature,
    ) -> Self {
        let mut transaction = Self::unsigned(sender, receiver, amount, nonce);
        transaction.signature = Some(signature);
        transaction
    }

    pub fn unsigned(sender: String, receiver: String, amount: Amount, nonce: u64) -> Self {
        let id = Self::compute_id(&sender, &receiver, amount, nonce);
        Transaction {
            id,
            sender,
            receiver,
            amount,
            nonce,
            signature: None,
        }
    }

    pub fn signing_payload(&self) -> Vec<u8> {
        signing_payload(&self.sender, &self.receiver, self.amount, self.nonce)
    }

    pub fn compute_id(sender: &str, receiver: &str, amount: Amount, nonce: u64) -> String {
        let payload = signing_payload(sender, receiver, amount, nonce);
        hex::encode(Sha256::digest(payload))
    }

    /// The id is a function of the content, so tampering is detectable even
    /// before checking the signature.
    pub fn has_valid_id(&self) -> bool {
        self.id == Self::compute_id(&self.sender, &self.receiver, self.amount, self.nonce)
    }

    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        match &self.signature {
            Some(signature) => {
                self.has_valid_id()
                    && verifying_key
                        .verify(&self.signing_payload(), signature)
                        .is_ok()
            }
            None => false,
        }
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Transaction: {} -> {} ({}, nonce: {}, id: {})",
            self.sender, self.receiver, self.amount, self.nonce, self.id
        )
    }
}

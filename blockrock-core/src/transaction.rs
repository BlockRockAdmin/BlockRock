use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Amounts are integers in the smallest indivisible unit: a ledger must never
/// accumulate floating point rounding errors.
pub type Amount = u64;

const TRANSACTION_DOMAIN: &[u8] = b"blockrock.transaction.v1";
/// Marks the start of a payload inside the signed bytes. A transaction without
/// payload appends nothing at all, so it keeps exactly the bytes it was signed
/// over before payloads existed — le catene già sigillate restano valide.
const PAYLOAD_TAG: u8 = 0x01;

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
    /// Dato arbitrario firmato insieme al resto: è così che una lettura di un
    /// sensore finisce in catena senza poter essere alterata. `None` per un
    /// semplice trasferimento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    pub signature: Option<Signature>,
}

/// Canonical bytes a sender signs. Every variable-length field is
/// length-prefixed, so ("ab", "c") and ("a", "bc") cannot collide.
fn signing_payload(
    sender: &str,
    receiver: &str,
    amount: Amount,
    nonce: u64,
    data: Option<&str>,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(TRANSACTION_DOMAIN.len() + sender.len() + receiver.len() + 32);
    payload.extend_from_slice(TRANSACTION_DOMAIN);
    for field in [sender.as_bytes(), receiver.as_bytes()] {
        payload.extend_from_slice(&(field.len() as u64).to_le_bytes());
        payload.extend_from_slice(field);
    }
    payload.extend_from_slice(&amount.to_le_bytes());
    payload.extend_from_slice(&nonce.to_le_bytes());
    // Niente viene aggiunto quando non c'è payload: una transazione firmata
    // prima che i payload esistessero produce ancora gli stessi byte. Il tag
    // rende `Some("")` distinguibile da `None`.
    if let Some(data) = data {
        payload.push(PAYLOAD_TAG);
        payload.extend_from_slice(&(data.len() as u64).to_le_bytes());
        payload.extend_from_slice(data.as_bytes());
    }
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
        Self::new_with_payload(sender, receiver, amount, nonce, None, signing_key)
    }

    /// Come [`new`], ma la transazione porta un dato firmato insieme al resto.
    pub fn new_with_payload(
        sender: String,
        receiver: String,
        amount: Amount,
        nonce: u64,
        payload: Option<String>,
        signing_key: &SigningKey,
    ) -> Self {
        let mut transaction = Self::unsigned_with_payload(sender, receiver, amount, nonce, payload);
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
        Self::signed_with_payload(sender, receiver, amount, nonce, None, signature)
    }

    /// Come [`signed`], per una transazione che porta un dato.
    pub fn signed_with_payload(
        sender: String,
        receiver: String,
        amount: Amount,
        nonce: u64,
        payload: Option<String>,
        signature: Signature,
    ) -> Self {
        let mut transaction = Self::unsigned_with_payload(sender, receiver, amount, nonce, payload);
        transaction.signature = Some(signature);
        transaction
    }

    pub fn unsigned(sender: String, receiver: String, amount: Amount, nonce: u64) -> Self {
        Self::unsigned_with_payload(sender, receiver, amount, nonce, None)
    }

    pub fn unsigned_with_payload(
        sender: String,
        receiver: String,
        amount: Amount,
        nonce: u64,
        payload: Option<String>,
    ) -> Self {
        let id = Self::compute_id(&sender, &receiver, amount, nonce, payload.as_deref());
        Transaction {
            id,
            sender,
            receiver,
            amount,
            nonce,
            payload,
            signature: None,
        }
    }

    pub fn signing_payload(&self) -> Vec<u8> {
        signing_payload(
            &self.sender,
            &self.receiver,
            self.amount,
            self.nonce,
            self.payload.as_deref(),
        )
    }

    pub fn compute_id(
        sender: &str,
        receiver: &str,
        amount: Amount,
        nonce: u64,
        payload: Option<&str>,
    ) -> String {
        let bytes = signing_payload(sender, receiver, amount, nonce, payload);
        hex::encode(Sha256::digest(bytes))
    }

    /// The id is a function of the content, so tampering is detectable even
    /// before checking the signature.
    pub fn has_valid_id(&self) -> bool {
        self.id
            == Self::compute_id(
                &self.sender,
                &self.receiver,
                self.amount,
                self.nonce,
                self.payload.as_deref(),
            )
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

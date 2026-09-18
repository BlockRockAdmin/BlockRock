use super::blockchain::ChainError;
use super::transaction::{Amount, Transaction, MAX_PAYLOAD_BYTES};
use ed25519_dalek::VerifyingKey;
use std::collections::HashMap;

/// A pool of validated transactions waiting to be sealed into a block.
///
/// The mempool is **not** persisted: on restart pending transactions are lost,
/// which is the expected behaviour for a best-effort submission layer.
/// Byte di transazioni oltre i quali conviene sigillare subito invece di
/// aspettare il prossimo tick. Con transazioni ordinarie sono qualche centinaio
/// di elementi per blocco.
pub const SEAL_THRESHOLD_BYTES: usize = 64 * 1024;

/// Quanti byte il mempool accetta di tenere in attesa. Finche' ogni transazione
/// veniva sigillata all'istante il pool non cresceva mai; ora che si accumula,
/// senza questo tetto sarebbe memoria illimitata offerta a chiunque sappia
/// firmare.
pub const MAX_MEMPOOL_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct Mempool {
    /// Transactions indexed by their id so duplicates are rejected in O(1).
    pending: HashMap<String, Transaction>,
    /// Somma dei pesi di `pending`, tenuta aggiornata per non doverla
    /// ricalcolare a ogni invio.
    bytes: usize,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            bytes: 0,
        }
    }

    /// Byte di transazioni attualmente in attesa.
    pub fn weight(&self) -> usize {
        self.bytes
    }

    /// `true` quando il lotto e' abbastanza pieno da valere un blocco subito,
    /// senza aspettare il tick.
    pub fn is_worth_sealing(&self) -> bool {
        self.bytes >= SEAL_THRESHOLD_BYTES
    }

    /// Number of transactions currently waiting.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Validates `transaction` against the supplied chain state and, if it
    /// passes, adds it to the pool. Returns the transaction id on success.
    ///
    /// The caller provides the chain data the validation needs, avoiding a
    /// borrow on the whole `Blockchain` (which owns this mempool).
    pub fn queue(
        &mut self,
        transaction: Transaction,
        public_keys: &HashMap<String, VerifyingKey>,
        balances: &HashMap<String, Amount>,
        nonces: &HashMap<String, u64>,
        tx_exists_on_chain: bool,
    ) -> Result<String, ChainError> {
        // 1. Id must match the content.
        if !transaction.has_valid_id() {
            return Err(ChainError::TamperedTransaction(transaction.id));
        }

        // 1b. Il dato allegato deve stare nel limite: e' la stessa regola che
        //     applica la validazione della catena, ma qui fallisce subito
        //     invece di far scoprire il problema al momento di sigillare.
        if !transaction.payload_within_limit() {
            return Err(ChainError::PayloadTooLarge {
                size: transaction.payload.as_ref().map_or(0, |data| data.len()),
                id: transaction.id,
                max: MAX_PAYLOAD_BYTES,
            });
        }

        // 2. Duplicate detection: already on chain or already in the pool.
        if tx_exists_on_chain {
            return Err(ChainError::DuplicateTransaction(transaction.id));
        }
        if self.pending.contains_key(&transaction.id) {
            return Err(ChainError::DuplicateTransaction(transaction.id));
        }

        // 3. Sender must have a registered public key.
        let public_key = public_keys
            .get(&transaction.sender)
            .ok_or_else(|| ChainError::UnknownSender(transaction.sender.clone()))?;

        // 4. Signature must verify.
        if !transaction.verify(public_key) {
            return Err(ChainError::InvalidSignature(transaction.id));
        }

        // 5. Il nonce deve essere il prossimo della sequenza, contando anche
        //    quelle gia' in attesa di questo mittente. Guardare solo la catena
        //    limitava ogni mittente a una transazione per blocco: il controllo
        //    sui fondi qui sotto somma da sempre gli importi gia' accodati
        //    dello stesso mittente, quindi la coda per mittente era prevista —
        //    era questo controllo a impedirla.
        let queued_from_sender = self
            .pending
            .values()
            .filter(|pending| pending.sender == transaction.sender)
            .count() as u64;
        let expected_nonce =
            nonces.get(&transaction.sender).copied().unwrap_or(0) + queued_from_sender;
        if transaction.nonce != expected_nonce {
            return Err(ChainError::InvalidNonce {
                sender: transaction.sender.clone(),
                expected: expected_nonce,
                found: transaction.nonce,
            });
        }

        // 6. Sender must have enough funds (considering already-queued
        //    transactions from the same sender).
        let already_spent: u64 = self
            .pending
            .values()
            .filter(|tx| tx.sender == transaction.sender)
            .map(|tx| tx.amount)
            .sum();
        let on_chain_balance = balances.get(&transaction.sender).copied().unwrap_or(0);
        let available = on_chain_balance.saturating_sub(already_spent);
        if available < transaction.amount {
            return Err(ChainError::InsufficientFunds {
                sender: transaction.sender.clone(),
                balance: available,
                amount: transaction.amount,
            });
        }

        // 7. Il pool deve avere spazio. Ultimo controllo di proposito: una
        //    transazione invalida va respinta per quello che e', non perche'
        //    e' arrivata quando il pool era pieno.
        let weight = transaction.weight();
        if self.bytes + weight > MAX_MEMPOOL_BYTES {
            return Err(ChainError::MempoolFull {
                queued: self.bytes,
                max: MAX_MEMPOOL_BYTES,
            });
        }

        let id = transaction.id.clone();
        self.pending.insert(id.clone(), transaction);
        self.bytes += weight;
        Ok(id)
    }

    /// Removes and returns all pending transactions, sorted by nonce within
    /// each sender so that they can be applied in order.
    pub fn drain(&mut self) -> Vec<Transaction> {
        let mut transactions: Vec<Transaction> = self.pending.drain().map(|(_, tx)| tx).collect();
        // Il peso segue il contenuto: dimenticarlo qui farebbe credere al pool
        // di essere pieno per sempre.
        self.bytes = 0;
        // Sort by (sender, nonce) so that transactions from the same sender
        // are applied in the correct order within a block.
        transactions.sort_by(|a, b| {
            a.sender
                .cmp(&b.sender)
                .then_with(|| a.nonce.cmp(&b.nonce))
        });
        transactions
    }

    /// Returns a reference to the pending transactions (for inspection).
    pub fn pending_transactions(&self) -> impl Iterator<Item = &Transaction> {
        self.pending.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::Blockchain;
    use crate::transaction::Transaction;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn alice_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    /// Helper: extracts the fields Mempool::queue needs from a Blockchain.
    fn chain_data(
        chain: &Blockchain,
    ) -> (
        &HashMap<String, VerifyingKey>,
        &HashMap<String, Amount>,
        &HashMap<String, u64>,
    ) {
        (&chain.public_keys, &chain.balances, &chain.nonces)
    }

    #[test]
    fn queue_valid_transaction() {
        let key = alice_key();
        let mut chain = Blockchain::new_single("test".to_string());
        chain.add_public_key("Alice", key.verifying_key());

        let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 30, 0, &key);
        let mut pool = Mempool::new();
        let (pk, bal, nonce) = chain_data(&chain);
        let id = pool.queue(tx, pk, bal, nonce, false).unwrap();
        assert_eq!(pool.len(), 1);
        assert!(pool.pending.contains_key(&id));
    }

    #[test]
    fn reject_duplicate_in_pool() {
        let key = alice_key();
        let mut chain = Blockchain::new_single("test".to_string());
        chain.add_public_key("Alice", key.verifying_key());

        let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 30, 0, &key);
        let mut pool = Mempool::new();
        let (pk, bal, nonce) = chain_data(&chain);
        pool.queue(tx.clone(), pk, bal, nonce, false).unwrap();
        let result = pool.queue(tx, pk, bal, nonce, false);
        assert!(result.is_err());
    }

    #[test]
    fn reject_insufficient_funds() {
        let key = alice_key();
        let mut chain = Blockchain::new_single("test".to_string());
        chain.add_public_key("Alice", key.verifying_key());
        // Alice starts with 100, try to send 200
        let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 200, 0, &key);
        let mut pool = Mempool::new();
        let (pk, bal, nonce) = chain_data(&chain);
        let result = pool.queue(tx, pk, bal, nonce, false);
        assert!(result.is_err());
    }

    #[test]
    fn reject_wrong_nonce() {
        let key = alice_key();
        let mut chain = Blockchain::new_single("test".to_string());
        chain.add_public_key("Alice", key.verifying_key());
        // Nonce should be 0, but we send 5
        let tx = Transaction::new("Alice".to_string(), "Bob".to_string(), 10, 5, &key);
        let mut pool = Mempool::new();
        let (pk, bal, nonce) = chain_data(&chain);
        let result = pool.queue(tx, pk, bal, nonce, false);
        assert!(result.is_err());
    }

    #[test]
    fn drain_returns_sorted() {
        let key = alice_key();
        let mut chain = Blockchain::new_single("test".to_string());
        chain.add_public_key("Alice", key.verifying_key());
        chain.add_public_key("Bob", key.verifying_key());

        let tx1 = Transaction::new("Bob".to_string(), "Charlie".to_string(), 10, 0, &key);
        let tx2 = Transaction::new("Alice".to_string(), "Charlie".to_string(), 20, 0, &key);

        let mut pool = Mempool::new();
        let (pk, bal, nonce) = chain_data(&chain);
        pool.queue(tx1, pk, bal, nonce, false).unwrap();
        pool.queue(tx2, pk, bal, nonce, false).unwrap();

        let drained = pool.drain();
        assert_eq!(pool.len(), 0);
        // Alice < Bob alphabetically, so Alice's tx should come first
        assert_eq!(drained[0].sender, "Alice");
        assert_eq!(drained[1].sender, "Bob");
    }

    #[test]
    fn queue_considers_already_spent() {
        let key = alice_key();
        let mut chain = Blockchain::new_single("test".to_string());
        chain.add_public_key("Alice", key.verifying_key());
        // Alice has 100. Queue 60, then try to queue another 60 → should fail.
        let tx1 = Transaction::new("Alice".to_string(), "Bob".to_string(), 60, 0, &key);
        let tx2 = Transaction::new("Alice".to_string(), "Charlie".to_string(), 60, 1, &key);

        let mut pool = Mempool::new();
        let (pk, bal, nonce) = chain_data(&chain);
        pool.queue(tx1, pk, bal, nonce, false).unwrap();
        let result = pool.queue(tx2, pk, bal, nonce, false);
        assert!(result.is_err());
    }
}
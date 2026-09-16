use super::blockchain::ChainError;
use super::transaction::{Amount, Transaction};
use ed25519_dalek::VerifyingKey;
use std::collections::HashMap;

/// A pool of validated transactions waiting to be sealed into a block.
///
/// The mempool is **not** persisted: on restart pending transactions are lost,
/// which is the expected behaviour for a best-effort submission layer.
#[derive(Debug, Clone, Default)]
pub struct Mempool {
    /// Transactions indexed by their id so duplicates are rejected in O(1).
    pending: HashMap<String, Transaction>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
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

        // 5. Nonce must be exactly the next expected one.
        let expected_nonce = nonces.get(&transaction.sender).copied().unwrap_or(0);
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

        let id = transaction.id.clone();
        self.pending.insert(id.clone(), transaction);
        Ok(id)
    }

    /// Removes and returns all pending transactions, sorted by nonce within
    /// each sender so that they can be applied in order.
    pub fn drain(&mut self) -> Vec<Transaction> {
        let mut transactions: Vec<Transaction> = self.pending.drain().map(|(_, tx)| tx).collect();
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
        let mut chain = Blockchain::new("test".to_string());
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
        let mut chain = Blockchain::new("test".to_string());
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
        let mut chain = Blockchain::new("test".to_string());
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
        let mut chain = Blockchain::new("test".to_string());
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
        let mut chain = Blockchain::new("test".to_string());
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
        let mut chain = Blockchain::new("test".to_string());
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
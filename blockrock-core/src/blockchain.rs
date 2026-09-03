use super::block::Block;
use super::transaction::{Amount, Transaction};
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::Path;

/// Account allowed to create value out of nothing. Blocks are sealed by an
/// authority, so only an authority can mint; transactions from this account
/// carry no signature, no nonce and no balance deduction.
pub const MINT_ACCOUNT: &str = "System";

/// Genesis carries a fixed timestamp so that every node building the same
/// initial allocation ends up with the same genesis hash and the chains stay
/// comparable.
const GENESIS_TIMESTAMP: u64 = 0;

#[derive(Debug, Clone, PartialEq)]
pub enum ChainError {
    UnauthorizedAuthority(String),
    UnknownAuthorityKey(String),
    AuthorityKeyMismatch(String),
    UnsealedBlock(u32),
    InvalidSeal(u32),
    WrongIndex { expected: u32, found: u32 },
    BrokenLink(u32),
    CorruptedHash(u32),
    GenesisMismatch,
    DuplicateTransaction(String),
    TamperedTransaction(String),
    UnknownSender(String),
    InvalidSignature(String),
    InvalidNonce {
        sender: String,
        expected: u64,
        found: u64,
    },
    InsufficientFunds {
        sender: String,
        balance: Amount,
        amount: Amount,
    },
    BalanceOverflow(String),
}

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChainError::UnauthorizedAuthority(name) => {
                write!(f, "'{}' is not an authorized authority", name)
            }
            ChainError::UnknownAuthorityKey(name) => {
                write!(f, "no public key registered for authority '{}'", name)
            }
            ChainError::AuthorityKeyMismatch(name) => {
                write!(f, "signing key does not match the key registered for '{}'", name)
            }
            ChainError::UnsealedBlock(index) => write!(f, "block {} carries no seal", index),
            ChainError::InvalidSeal(index) => write!(f, "block {} has an invalid seal", index),
            ChainError::WrongIndex { expected, found } => {
                write!(f, "expected block index {}, found {}", expected, found)
            }
            ChainError::BrokenLink(index) => {
                write!(f, "block {} does not link to the previous hash", index)
            }
            ChainError::CorruptedHash(index) => write!(f, "block {} hash does not match its content", index),
            ChainError::GenesisMismatch => write!(f, "the chains do not share the same genesis block"),
            ChainError::DuplicateTransaction(id) => write!(f, "transaction {} is already on chain", id),
            ChainError::TamperedTransaction(id) => {
                write!(f, "transaction {} id does not match its content", id)
            }
            ChainError::UnknownSender(sender) => write!(f, "no public key registered for '{}'", sender),
            ChainError::InvalidSignature(id) => write!(f, "transaction {} has an invalid signature", id),
            ChainError::InvalidNonce {
                sender,
                expected,
                found,
            } => write!(
                f,
                "'{}' expected nonce {}, found {}",
                sender, expected, found
            ),
            ChainError::InsufficientFunds {
                sender,
                balance,
                amount,
            } => write!(
                f,
                "'{}' holds {} but tried to send {}",
                sender, balance, amount
            ),
            ChainError::BalanceOverflow(account) => write!(f, "balance overflow for '{}'", account),
        }
    }
}

impl std::error::Error for ChainError {}

/// Balances and nonces derived by replaying blocks over the initial allocation.
#[derive(Debug, Clone, Default)]
struct LedgerState {
    balances: HashMap<String, Amount>,
    nonces: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blockchain {
    pub blocks: Vec<Block>,
    /// Authority this node seals its own blocks with.
    pub authority: String,
    /// Every authority allowed to seal a block on this chain.
    #[serde(default)]
    pub authorities: HashSet<String>,
    pub balances: HashMap<String, Amount>,
    /// Allocation the chain starts from, kept so the whole state can be
    /// recomputed by replaying blocks (needed to adopt a peer's chain).
    #[serde(default)]
    pub initial_balances: HashMap<String, Amount>,
    /// Next nonce expected from each sender.
    #[serde(default)]
    pub nonces: HashMap<String, u64>,
    pub public_keys: HashMap<String, VerifyingKey>,
    /// Runtime-only index: transaction queries must not scan the entire chain.
    #[serde(skip)]
    transaction_index: HashMap<String, (usize, usize)>,
}

impl Blockchain {
    pub fn new(authority: String) -> Self {
        let initial_balances = HashMap::from([
            (MINT_ACCOUNT.to_string(), 1000),
            ("Alice".to_string(), 100),
            ("Bob".to_string(), 50),
            ("Charlie".to_string(), 30),
            ("Node1".to_string(), 50),
            ("Node2".to_string(), 0),
        ]);

        let genesis = Block::with_timestamp(
            0,
            Vec::new(),
            "0".to_string(),
            authority.clone(),
            GENESIS_TIMESTAMP,
        );

        let mut blockchain = Blockchain {
            blocks: vec![genesis],
            authorities: HashSet::from([authority.clone()]),
            authority,
            balances: initial_balances.clone(),
            initial_balances,
            nonces: HashMap::new(),
            public_keys: HashMap::new(),
            transaction_index: HashMap::new(),
        };
        blockchain.rebuild_transaction_index();
        blockchain
    }

    /// Authorizes `name` to seal blocks and records the key its seals are
    /// verified against.
    pub fn register_authority(&mut self, name: &str, key: VerifyingKey) {
        self.authorities.insert(name.to_string());
        self.public_keys.insert(name.to_string(), key);
    }

    pub fn add_public_key(&mut self, name: &str, key: VerifyingKey) {
        self.public_keys.insert(name.to_string(), key);
    }

    pub fn public_key(&self, name: &str) -> Option<&VerifyingKey> {
        self.public_keys.get(name)
    }

    /// Nonce the next transaction from `account` must carry.
    pub fn next_nonce(&self, account: &str) -> u64 {
        self.nonces.get(account).copied().unwrap_or(0)
    }

    pub fn balance_of(&self, account: &str) -> Amount {
        self.balances.get(account).copied().unwrap_or(0)
    }

    pub fn genesis_hash(&self) -> Option<&str> {
        self.blocks.first().map(|block| block.hash.as_str())
    }

    /// Seals `transactions` into a new block. The signing key must belong to an
    /// authorized authority, which is what makes this proof of authority.
    pub fn add_block(
        &mut self,
        transactions: Vec<Transaction>,
        authority: &str,
        signing_key: &SigningKey,
    ) -> Result<u32, ChainError> {
        if !self.authorities.contains(authority) {
            return Err(ChainError::UnauthorizedAuthority(authority.to_string()));
        }
        let registered = self
            .public_keys
            .get(authority)
            .ok_or_else(|| ChainError::UnknownAuthorityKey(authority.to_string()))?;
        if registered != &signing_key.verifying_key() {
            return Err(ChainError::AuthorityKeyMismatch(authority.to_string()));
        }

        let index = self.blocks.len() as u32;
        let previous_hash = self.last_hash();
        let block = Block::sealed(
            index,
            transactions,
            previous_hash,
            authority.to_string(),
            signing_key,
        );
        self.try_append_block(block)?;
        Ok(index)
    }

    /// Validates a block produced elsewhere (typically received from a peer)
    /// and appends it to the tip.
    pub fn try_append_block(&mut self, block: Block) -> Result<(), ChainError> {
        let expected_index = self.blocks.len() as u32;
        let previous_hash = self.last_hash();
        let mut state = self.current_state();
        let mut seen: HashSet<String> = self.transaction_index.keys().cloned().collect();

        verify_block(
            &self.authorities,
            &self.public_keys,
            &block,
            expected_index,
            &previous_hash,
            &mut state,
            &mut seen,
        )?;

        self.commit_block(block, state);
        Ok(())
    }

    /// Adopts a peer's chain when it is longer, shares our genesis and is
    /// valid from block 0. Returns whether the local chain was replaced.
    pub fn try_adopt_chain(&mut self, blocks: Vec<Block>) -> Result<bool, ChainError> {
        if blocks.len() <= self.blocks.len() {
            return Ok(false);
        }
        match (self.genesis_hash(), blocks.first()) {
            (Some(ours), Some(theirs)) if ours != theirs.hash => {
                return Err(ChainError::GenesisMismatch)
            }
            (_, None) => return Ok(false),
            _ => {}
        }

        let state = self.replay(&blocks)?;
        self.blocks = blocks;
        self.balances = state.balances;
        self.nonces = state.nonces;
        self.rebuild_transaction_index();
        Ok(true)
    }

    pub fn get_blocks(&self) -> Vec<Block> {
        self.blocks.clone()
    }

    pub fn get_balances(&self) -> Vec<(String, Amount)> {
        let mut balances: Vec<(String, Amount)> =
            self.balances.iter().map(|(k, v)| (k.clone(), *v)).collect();
        balances.sort_by(|a, b| a.0.cmp(&b.0));
        balances
    }

    pub fn get_transaction(&self, id: &str) -> Option<Transaction> {
        let (block_index, transaction_index) = self.transaction_index.get(id)?;
        self.blocks
            .get(*block_index)?
            .transactions
            .get(*transaction_index)
            .cloned()
    }

    /// Replays the whole chain: hashes, links, seals, signatures, nonces and
    /// balances all have to hold.
    pub fn validate(&self) -> Result<(), ChainError> {
        self.replay(&self.blocks).map(|_| ())
    }

    pub fn validate_chain(&self) -> bool {
        self.validate().is_ok()
    }

    /// Salva la blockchain in formato JSON, scrivendo prima un file temporaneo
    /// così un'interruzione non lascia una catena troncata su disco.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        let temporary = Path::new(&temporary);
        fs::write(temporary, data)?;
        fs::rename(temporary, path)
    }

    /// Carica una blockchain da file JSON
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let data = fs::read_to_string(path)?;
        let mut chain: Blockchain = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        chain.rebuild_transaction_index();
        Ok(chain)
    }

    fn last_hash(&self) -> String {
        self.blocks
            .last()
            .map(|block| block.hash.clone())
            .unwrap_or_else(|| "0".to_string())
    }

    fn current_state(&self) -> LedgerState {
        LedgerState {
            balances: self.balances.clone(),
            nonces: self.nonces.clone(),
        }
    }

    fn commit_block(&mut self, block: Block, state: LedgerState) {
        let block_index = self.blocks.len();
        self.blocks.push(block);
        for (transaction_index, transaction) in
            self.blocks[block_index].transactions.iter().enumerate()
        {
            self.transaction_index
                .insert(transaction.id.clone(), (block_index, transaction_index));
        }
        self.balances = state.balances;
        self.nonces = state.nonces;
    }

    fn replay(&self, blocks: &[Block]) -> Result<LedgerState, ChainError> {
        let mut state = LedgerState {
            balances: self.initial_balances.clone(),
            nonces: HashMap::new(),
        };
        let mut seen = HashSet::new();
        let mut previous_hash = "0".to_string();

        for (index, block) in blocks.iter().enumerate() {
            verify_block(
                &self.authorities,
                &self.public_keys,
                block,
                index as u32,
                &previous_hash,
                &mut state,
                &mut seen,
            )?;
            previous_hash = block.hash.clone();
        }
        Ok(state)
    }

    fn rebuild_transaction_index(&mut self) {
        self.transaction_index.clear();
        for (block_index, block) in self.blocks.iter().enumerate() {
            for (transaction_index, transaction) in block.transactions.iter().enumerate() {
                self.transaction_index
                    .insert(transaction.id.clone(), (block_index, transaction_index));
            }
        }
    }
}

/// Verifies one block against the state accumulated so far and applies it.
/// `state` and `seen` are only mutated on success paths, and callers work on a
/// copy, so a rejected block leaves the ledger untouched.
fn verify_block(
    authorities: &HashSet<String>,
    public_keys: &HashMap<String, VerifyingKey>,
    block: &Block,
    expected_index: u32,
    previous_hash: &str,
    state: &mut LedgerState,
    seen: &mut HashSet<String>,
) -> Result<(), ChainError> {
    if block.index != expected_index {
        return Err(ChainError::WrongIndex {
            expected: expected_index,
            found: block.index,
        });
    }
    if block.previous_hash != previous_hash {
        return Err(ChainError::BrokenLink(block.index));
    }
    if !block.is_hash_valid() {
        return Err(ChainError::CorruptedHash(block.index));
    }

    // Genesis has no authority to seal it: it is the root of trust itself.
    if block.index > 0 {
        if !authorities.contains(&block.authority) {
            return Err(ChainError::UnauthorizedAuthority(block.authority.clone()));
        }
        let key = public_keys
            .get(&block.authority)
            .ok_or_else(|| ChainError::UnknownAuthorityKey(block.authority.clone()))?;
        if block.authority_signature.is_none() {
            return Err(ChainError::UnsealedBlock(block.index));
        }
        if !block.verify_seal(key) {
            return Err(ChainError::InvalidSeal(block.index));
        }
    }

    for transaction in &block.transactions {
        apply_transaction(public_keys, state, seen, transaction)?;
    }
    Ok(())
}

/// Validates a single transaction against the running state and applies it.
/// Balances and nonces move as we go, so two transfers in the same block
/// cannot both spend the same funds.
fn apply_transaction(
    public_keys: &HashMap<String, VerifyingKey>,
    state: &mut LedgerState,
    seen: &mut HashSet<String>,
    transaction: &Transaction,
) -> Result<(), ChainError> {
    if !transaction.has_valid_id() {
        return Err(ChainError::TamperedTransaction(transaction.id.clone()));
    }
    if !seen.insert(transaction.id.clone()) {
        return Err(ChainError::DuplicateTransaction(transaction.id.clone()));
    }

    if transaction.sender != MINT_ACCOUNT {
        let key = public_keys
            .get(&transaction.sender)
            .ok_or_else(|| ChainError::UnknownSender(transaction.sender.clone()))?;
        if !transaction.verify(key) {
            return Err(ChainError::InvalidSignature(transaction.id.clone()));
        }

        let expected = state
            .nonces
            .get(&transaction.sender)
            .copied()
            .unwrap_or(0);
        if transaction.nonce != expected {
            return Err(ChainError::InvalidNonce {
                sender: transaction.sender.clone(),
                expected,
                found: transaction.nonce,
            });
        }

        let balance = state
            .balances
            .get(&transaction.sender)
            .copied()
            .unwrap_or(0);
        if balance < transaction.amount {
            return Err(ChainError::InsufficientFunds {
                sender: transaction.sender.clone(),
                balance,
                amount: transaction.amount,
            });
        }

        state
            .balances
            .insert(transaction.sender.clone(), balance - transaction.amount);
        state
            .nonces
            .insert(transaction.sender.clone(), expected + 1);
    }

    let credited = state
        .balances
        .get(&transaction.receiver)
        .copied()
        .unwrap_or(0)
        .checked_add(transaction.amount)
        .ok_or_else(|| ChainError::BalanceOverflow(transaction.receiver.clone()))?;
    state.balances.insert(transaction.receiver.clone(), credited);

    Ok(())
}

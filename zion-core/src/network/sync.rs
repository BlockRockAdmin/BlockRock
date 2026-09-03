//! Chain synchronisation between peers.
//!
//! The wire messages and the rules for applying them live here, away from the
//! swarm plumbing, so the interesting part can be tested without a network.

use blockrock_core::block::Block;
use blockrock_core::blockchain::{Blockchain, ChainError};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sender keys travel with blocks: a peer cannot verify a transaction whose
/// sender it never saw registered. Registering accounts on chain would remove
/// the need for this, and is the natural next step.
pub type Accounts = HashMap<String, VerifyingKey>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockAnnouncement {
    pub block: Block,
    pub accounts: Accounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSnapshot {
    pub blocks: Vec<Block>,
    pub accounts: Accounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncRequest {
    /// "Send me your chain", asked of every peer we discover.
    GetChain,
    /// "I just sealed this", pushed to every connected peer.
    NewBlock(BlockAnnouncement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    Chain(ChainSnapshot),
    Ack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockOutcome {
    /// Appended to our tip.
    Appended,
    /// We already hold this block.
    Known,
    /// It does not sit at our height: only the full chain can settle it.
    OutOfOrder,
}

pub fn snapshot_of(chain: &Blockchain) -> ChainSnapshot {
    ChainSnapshot {
        blocks: chain.get_blocks(),
        accounts: chain.public_keys.clone(),
    }
}

pub fn announcement_of(chain: &Blockchain, block: Block) -> BlockAnnouncement {
    BlockAnnouncement {
        block,
        accounts: chain.public_keys.clone(),
    }
}

/// Applies a block announced by a peer. Blocks that do not extend our tip are
/// reported as [`BlockOutcome::OutOfOrder`] so the caller can ask for the
/// peer's whole chain instead of guessing.
pub fn apply_announcement(
    chain: &mut Blockchain,
    announcement: BlockAnnouncement,
) -> Result<BlockOutcome, ChainError> {
    let BlockAnnouncement { block, accounts } = announcement;
    let height = chain.blocks.len() as u32;

    if block.index < height {
        let known = chain
            .blocks
            .get(block.index as usize)
            .map(|ours| ours.hash == block.hash)
            .unwrap_or(false);
        return Ok(if known {
            BlockOutcome::Known
        } else {
            BlockOutcome::OutOfOrder
        });
    }
    if block.index > height {
        return Ok(BlockOutcome::OutOfOrder);
    }

    merge_accounts(chain, accounts);
    chain.try_append_block(block)?;
    Ok(BlockOutcome::Appended)
}

/// Adopts a peer's chain when it is longer and valid. Returns whether the
/// local chain was replaced.
pub fn apply_snapshot(chain: &mut Blockchain, snapshot: ChainSnapshot) -> Result<bool, ChainError> {
    if snapshot.blocks.len() <= chain.blocks.len() {
        return Ok(false);
    }
    merge_accounts(chain, snapshot.accounts);
    chain.try_adopt_chain(snapshot.blocks)
}

/// Learns account keys we do not have yet. An existing binding is never
/// overwritten: a peer cannot rebind a name we already know.
fn merge_accounts(chain: &mut Blockchain, accounts: Accounts) {
    for (name, key) in accounts {
        if chain.public_key(&name).is_none() {
            chain.add_public_key(&name, key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockrock_core::transaction::Transaction;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    struct Network {
        authority: SigningKey,
        alice: SigningKey,
    }

    impl Network {
        fn new() -> Self {
            Network {
                authority: SigningKey::generate(&mut OsRng),
                alice: SigningKey::generate(&mut OsRng),
            }
        }

        /// A node that trusts the authority but has never heard of Alice.
        fn node(&self) -> Blockchain {
            let mut chain = Blockchain::new("blockrock".to_string());
            chain.register_authority("blockrock", self.authority.verifying_key());
            chain
        }

        fn seal_transfer(&self, chain: &mut Blockchain, amount: u64, nonce: u64) {
            let tx = Transaction::new(
                "Alice".to_string(),
                "Bob".to_string(),
                amount,
                nonce,
                &self.alice,
            );
            chain
                .add_block(vec![tx], "blockrock", &self.authority)
                .unwrap();
        }
    }

    #[test]
    fn an_announced_block_is_appended_and_teaches_the_sender_key() {
        let network = Network::new();
        let mut author = network.node();
        author.add_public_key("Alice", network.alice.verifying_key());
        network.seal_transfer(&mut author, 20, 0);

        let mut receiver = network.node();
        let announcement = announcement_of(&author, author.blocks[1].clone());

        assert_eq!(
            apply_announcement(&mut receiver, announcement).unwrap(),
            BlockOutcome::Appended
        );
        assert_eq!(receiver.balance_of("Alice"), 80);
        assert!(receiver.public_key("Alice").is_some());
        assert!(receiver.validate_chain());
    }

    #[test]
    fn a_block_already_held_is_not_applied_twice() {
        let network = Network::new();
        let mut author = network.node();
        author.add_public_key("Alice", network.alice.verifying_key());
        network.seal_transfer(&mut author, 20, 0);

        let mut receiver = network.node();
        let announcement = announcement_of(&author, author.blocks[1].clone());
        apply_announcement(&mut receiver, announcement.clone()).unwrap();

        assert_eq!(
            apply_announcement(&mut receiver, announcement).unwrap(),
            BlockOutcome::Known
        );
        assert_eq!(receiver.blocks.len(), 2);
        assert_eq!(receiver.balance_of("Alice"), 80);
    }

    #[test]
    fn a_block_from_the_future_asks_for_the_whole_chain() {
        let network = Network::new();
        let mut author = network.node();
        author.add_public_key("Alice", network.alice.verifying_key());
        network.seal_transfer(&mut author, 10, 0);
        network.seal_transfer(&mut author, 10, 1);

        let mut receiver = network.node();
        let announcement = announcement_of(&author, author.blocks[2].clone());

        assert_eq!(
            apply_announcement(&mut receiver, announcement).unwrap(),
            BlockOutcome::OutOfOrder
        );
        assert_eq!(receiver.blocks.len(), 1);

        // Which is exactly what the snapshot settles.
        assert!(apply_snapshot(&mut receiver, snapshot_of(&author)).unwrap());
        assert_eq!(receiver.blocks.len(), 3);
        assert_eq!(receiver.balance_of("Alice"), 80);
    }

    #[test]
    fn a_shorter_snapshot_never_rewinds_us() {
        let network = Network::new();
        let mut author = network.node();
        author.add_public_key("Alice", network.alice.verifying_key());
        network.seal_transfer(&mut author, 10, 0);

        let behind = network.node();
        assert!(!apply_snapshot(&mut author, snapshot_of(&behind)).unwrap());
        assert_eq!(author.blocks.len(), 2);
    }

    #[test]
    fn a_peer_cannot_rebind_an_account_we_already_know() {
        let network = Network::new();
        let mut receiver = network.node();
        receiver.add_public_key("Alice", network.alice.verifying_key());

        let impostor = SigningKey::generate(&mut OsRng);
        let mut accounts = Accounts::new();
        accounts.insert("Alice".to_string(), impostor.verifying_key());
        merge_accounts(&mut receiver, accounts);

        assert_eq!(
            receiver.public_key("Alice"),
            Some(&network.alice.verifying_key())
        );
    }

    #[test]
    fn a_block_sealed_by_an_untrusted_key_is_rejected() {
        let network = Network::new();
        let mut author = network.node();
        author.add_public_key("Alice", network.alice.verifying_key());
        network.seal_transfer(&mut author, 20, 0);

        // A node that trusts a different authority key.
        let other = Network::new();
        let mut receiver = other.node();
        let announcement = announcement_of(&author, author.blocks[1].clone());

        assert_eq!(
            apply_announcement(&mut receiver, announcement),
            Err(ChainError::InvalidSeal(1))
        );
        assert_eq!(receiver.blocks.len(), 1);
    }
}

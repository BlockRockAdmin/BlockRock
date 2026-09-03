use blockrock_core::blockchain::Blockchain;
use std::io;
use std::path::{Path, PathBuf};

/// Where the chain lives on disk. Every accepted block is written through
/// before the request returns, so a restart resumes from the same tip instead
/// of starting a brand new chain.
#[derive(Debug, Clone)]
pub struct ChainStore {
    path: PathBuf,
}

impl ChainStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        ChainStore { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the persisted chain, or starts a new one whose genesis is sealed
    /// by `authority`.
    pub fn load_or_create(&self, authority: &str) -> io::Result<Blockchain> {
        match Blockchain::load_from_file(&self.path) {
            Ok(chain) => Ok(chain),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(Blockchain::new(authority.to_string()))
            }
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, chain: &Blockchain) -> io::Result<()> {
        chain.save_to_file(&self.path)
    }
}

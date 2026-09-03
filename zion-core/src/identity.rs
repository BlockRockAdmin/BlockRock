use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::fs;
use std::io;
use std::path::Path;

/// The authority this node seals its blocks with.
pub struct NodeIdentity {
    pub name: String,
    pub signing_key: SigningKey,
}

impl NodeIdentity {
    /// Loads the authority key, generating and persisting one on first run.
    /// The file holds the raw 32-byte ed25519 seed and is created private to
    /// the user. Nodes of a single-authority network share this file.
    pub fn load_or_create(name: String, path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let signing_key = match fs::read(path) {
            Ok(bytes) => {
                let seed: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} is not a 32-byte ed25519 seed", path.display()),
                    )
                })?;
                SigningKey::from_bytes(&seed)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let key = SigningKey::generate(&mut OsRng);
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent)?;
                    }
                }
                fs::write(path, key.to_bytes())?;
                restrict_permissions(path)?;
                key
            }
            Err(error) => return Err(error),
        };

        Ok(NodeIdentity { name, signing_key })
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

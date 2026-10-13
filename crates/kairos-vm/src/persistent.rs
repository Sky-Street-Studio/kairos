use std::collections::{BTreeMap, BTreeSet};

use kairos_script::Value;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 4] = b"KPER";
const VERSION: u32 = 1;
const PREFIX: &str = "persistent.";

/// Data shared by every playthrough of a game: `persistent.*` variables and the lines the player
/// has read. Rollback, saves and new games leave it alone.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Persistent {
    pub values: BTreeMap<String, Value>,
    /// Statement IDs of dialogue lines the player has moved past.
    pub seen: BTreeSet<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PersistentError {
    #[error("not a Kairos persistent data file")]
    NotPersistent,
    #[error("persistent data version {0} is newer than this engine supports")]
    UnsupportedVersion(u32),
    #[error("corrupt persistent data: {0}")]
    Corrupt(#[from] postcard::Error),
}

impl Persistent {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        out.extend(postcard::to_allocvec(&VERSION).expect("a version always serialises"));
        out.extend(postcard::to_allocvec(self).expect("persistent data is always serialisable"));
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Persistent, PersistentError> {
        let body = bytes.strip_prefix(MAGIC).ok_or(PersistentError::NotPersistent)?;
        let (version, body) = postcard::take_from_bytes::<u32>(body)?;
        if version > VERSION {
            return Err(PersistentError::UnsupportedVersion(version));
        }
        Ok(postcard::from_bytes(body)?)
    }
}

pub(crate) fn is_persistent(name: &str) -> bool {
    name.starts_with(PREFIX)
}

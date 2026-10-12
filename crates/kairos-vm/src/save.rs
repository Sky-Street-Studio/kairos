use std::collections::BTreeMap;
use std::sync::Arc;

use kairos_script::Value;
use serde::{Deserialize, Serialize};

use crate::program::Program;
use crate::vm::{RuntimeError, Stage, State, Track, Vm};

const MAGIC: &[u8; 4] = b"KSAV";
const VERSION: u32 = 1;

/// Instructions are referenced by stable ID, never by index, so a save outlives script edits
/// that do not touch the saved line or its call sites.
#[derive(Serialize, Deserialize)]
struct SaveData {
    version: u32,
    pc: String,
    stack: Vec<String>,
    store: BTreeMap<String, Value>,
    stage: Stage,
    audio: BTreeMap<String, Track>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("not a Kairos save file")]
    NotASave,
    #[error("save version {0} is newer than this engine supports")]
    UnsupportedVersion(u32),
    #[error("corrupt save: {0}")]
    Corrupt(#[from] postcard::Error),
    #[error("the saved line `{0}` no longer exists in the script")]
    MissingStatement(String),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl Vm {
    /// Serialises the current interaction, or `None` before the first one.
    pub fn save(&self) -> Option<Vec<u8>> {
        let state = self.history.last()?;
        let id = |pc: usize| self.program.ids[pc].clone();
        let data = SaveData {
            version: VERSION,
            pc: id(state.pc),
            stack: state.stack.iter().map(|&pc| id(pc)).collect(),
            store: state.store.clone(),
            stage: state.stage.clone(),
            audio: state.audio.clone(),
        };
        let mut out = MAGIC.to_vec();
        out.extend(postcard::to_allocvec(&data).expect("save data is always serialisable"));
        Some(out)
    }

    /// Restores a save. The next [`Vm::advance`] shows the saved interaction again.
    pub fn load(program: Arc<Program>, bytes: &[u8]) -> Result<Vm, LoadError> {
        let body = bytes.strip_prefix(MAGIC).ok_or(LoadError::NotASave)?;
        let data: SaveData = postcard::from_bytes(body)?;
        if data.version > VERSION {
            return Err(LoadError::UnsupportedVersion(data.version));
        }
        let pc_of = |id: &String| program.pc_of(id).ok_or_else(|| LoadError::MissingStatement(id.clone()));
        let pc = pc_of(&data.pc)?;
        let stack = data.stack.iter().map(pc_of).collect::<Result<_, _>>()?;
        let mut vm = Vm::new(program.clone())?;
        vm.state.store.extend(data.store);
        vm.state = State { pc, stack, stage: data.stage, audio: data.audio, menu: None, store: vm.state.store.clone() };
        Ok(vm)
    }
}

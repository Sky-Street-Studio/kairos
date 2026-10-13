use std::collections::BTreeMap;
use std::sync::Arc;

use kairos_script::Value;
use serde::{Deserialize, Serialize};

use crate::program::{Instr, Program};
use crate::vm::{RuntimeError, Stage, State, Track, Vm};

const MAGIC: &[u8; 4] = b"KSAV";
const VERSION: u32 = 2;
/// How many earlier statement IDs a save keeps for finding its place after a patch.
const CONTEXT: usize = 32;

/// A saved position: the statement's ID plus the IDs before it in the same label, nearest
/// first, so a load can find its place when a patch rewords the statement itself.
#[derive(Serialize, Deserialize)]
struct Position {
    id: String,
    before: Vec<String>,
}

/// Instructions are referenced by stable ID, never by index, so a save outlives script edits
/// that do not touch the saved line or its call sites.
#[derive(Serialize, Deserialize)]
struct SaveData {
    pc: Position,
    stack: Vec<Position>,
    store: BTreeMap<String, Value>,
    stage: Stage,
    audio: BTreeMap<String, Track>,
}

/// The version 1 layout, which stored bare IDs.
#[derive(Deserialize)]
struct SaveDataV1 {
    pc: String,
    stack: Vec<String>,
    store: BTreeMap<String, Value>,
    stage: Stage,
    audio: BTreeMap<String, Track>,
}

impl From<SaveDataV1> for SaveData {
    fn from(v1: SaveDataV1) -> SaveData {
        let position = |id: String| Position { id, before: Vec::new() };
        SaveData {
            pc: position(v1.pc),
            stack: v1.stack.into_iter().map(position).collect(),
            store: v1.store,
            stage: v1.stage,
            audio: v1.audio,
        }
    }
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

impl Program {
    fn position(&self, pc: usize) -> Position {
        let id = self.ids[pc].clone();
        let label = label_of(&id);
        let before = self.ids[..pc].iter().rev().take_while(|b| label_of(b) == label).take(CONTEXT).cloned().collect();
        Position { id, before }
    }

    /// Finds a saved position, or the statement after the nearest surviving earlier one, so
    /// nothing that already ran runs again. Returns whether it had to fall back.
    fn locate(&self, position: &Position) -> Result<(usize, bool), LoadError> {
        if let Some(pc) = self.pc_of(&position.id) {
            return Ok((pc, false));
        }
        position
            .before
            .iter()
            .find_map(|id| self.pc_of(id).map(|pc| pc + 1))
            .or_else(|| self.labels.get(label_of(&position.id)).copied())
            .filter(|&pc| pc < self.instrs.len())
            .map(|pc| (pc, true))
            .ok_or_else(|| LoadError::MissingStatement(position.id.clone()))
    }

    /// Like [`Program::locate`] for a call stack entry. Several edited lines in a row put the
    /// fallback before the call, and returning there would run the call a second time.
    fn locate_call(&self, position: &Position) -> Result<(usize, bool), LoadError> {
        let (pc, moved) = self.locate(position)?;
        if !moved {
            return Ok((pc, false));
        }
        let label = label_of(&self.ids[pc]);
        (pc..self.instrs.len())
            .take_while(|&at| label_of(&self.ids[at]) == label)
            .find(|&at| matches!(self.instrs[at], Instr::Call(_)))
            .map(|at| (at, true))
            .ok_or_else(|| LoadError::MissingStatement(position.id.clone()))
    }
}

fn label_of(id: &str) -> &str {
    id.split_once(':').map_or(id, |(label, _)| label)
}

impl Vm {
    /// Serialises the current interaction, or `None` before the first one.
    pub fn save(&self) -> Option<Vec<u8>> {
        let state = self.history.last()?;
        let data = SaveData {
            pc: self.program.position(state.pc),
            stack: state.stack.iter().map(|&pc| self.program.position(pc)).collect(),
            store: state.store.clone(),
            stage: state.stage.clone(),
            audio: state.audio.clone(),
        };
        let mut out = MAGIC.to_vec();
        out.extend(postcard::to_allocvec(&VERSION).expect("a version always serialises"));
        out.extend(postcard::to_allocvec(&data).expect("save data is always serialisable"));
        Some(out)
    }

    /// Restores a save. The next [`Vm::advance`] shows the saved interaction again.
    pub fn load(program: Arc<Program>, bytes: &[u8]) -> Result<Vm, LoadError> {
        let body = bytes.strip_prefix(MAGIC).ok_or(LoadError::NotASave)?;
        let (version, body) = postcard::take_from_bytes::<u32>(body)?;
        let data: SaveData = match version {
            1 => postcard::from_bytes::<SaveDataV1>(body)?.into(),
            VERSION => postcard::from_bytes(body)?,
            _ => return Err(LoadError::UnsupportedVersion(version)),
        };
        let (pc, mut relocated) = program.locate(&data.pc)?;
        let mut stack = Vec::with_capacity(data.stack.len());
        for position in &data.stack {
            let (call, moved) = program.locate_call(position)?;
            stack.push(call);
            relocated |= moved;
        }
        let mut vm = Vm::new(program.clone())?;
        vm.state.store.extend(data.store);
        vm.state = State { pc, stack, stage: data.stage, audio: data.audio, menu: None, store: vm.state.store.clone() };
        vm.relocated = relocated;
        Ok(vm)
    }

    /// Whether [`Vm::load`] could not find a saved line, because a patch changed it, and resumed
    /// after the nearest earlier line that survived.
    pub fn relocated(&self) -> bool {
        self.relocated
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::*;

    fn program(src: &str) -> Arc<Program> {
        let (stmts, _) = kairos_script::parse(src);
        Arc::new(crate::compile(&[stmts]).0)
    }

    #[test]
    fn version_1_saves_still_load() {
        #[derive(Serialize)]
        struct V1<'a> {
            version: u32,
            pc: &'a str,
            stack: Vec<String>,
            store: BTreeMap<String, Value>,
            stage: Stage,
            audio: BTreeMap<String, Track>,
        }
        let p = program("default n = 0\nlabel start:\n    \"One\"\n    \"Two\"\n");
        let v1 = V1 {
            version: 1,
            pc: &p.ids[1],
            stack: Vec::new(),
            store: BTreeMap::from([("n".into(), Value::Int(3))]),
            stage: Stage::default(),
            audio: BTreeMap::new(),
        };
        let mut bytes = MAGIC.to_vec();
        bytes.extend(postcard::to_allocvec(&v1).unwrap());

        let mut vm = Vm::load(p, &bytes).unwrap();
        assert_eq!(vm.variables()["n"], Value::Int(3));
        assert!(matches!(vm.advance().unwrap(), crate::Event::Say { text, .. } if text == "Two"));
    }

    #[test]
    fn saves_from_a_newer_engine_name_their_version() {
        let mut bytes = MAGIC.to_vec();
        bytes.extend(postcard::to_allocvec(&(VERSION + 1)).unwrap());
        bytes.extend([0xff; 8]);
        assert!(matches!(
            Vm::load(program("label start:\n    return\n"), &bytes),
            Err(LoadError::UnsupportedVersion(3))
        ));
    }
}

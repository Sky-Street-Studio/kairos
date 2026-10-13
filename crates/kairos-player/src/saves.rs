//! Save slots: a VM save plus a one-line preview per slot.

use std::collections::HashMap;

pub enum SaveStore {
    /// `slot-N.ksav` and `slot-N.txt` files in a per-project directory.
    #[cfg(not(target_arch = "wasm32"))]
    Dir(std::path::PathBuf),
    /// `localStorage` entries under a per-project prefix.
    #[cfg(target_arch = "wasm32")]
    Local(String),
    Memory {
        slots: HashMap<usize, (Vec<u8>, String, u128)>,
        language: Option<String>,
        persistent: Option<Vec<u8>>,
    },
}

impl SaveStore {
    /// The platform data directory, e.g. `~/Library/Application Support/Kairos/saves/<project>`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn for_project(name: &str) -> SaveStore {
        match directories::ProjectDirs::from("", "", "Kairos") {
            Some(dirs) => SaveStore::Dir(dirs.data_dir().join("saves").join(sanitize(name))),
            None => SaveStore::Memory { slots: HashMap::new(), language: None, persistent: None },
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn for_project(name: &str) -> SaveStore {
        SaveStore::Local(format!("kairos:{}:", sanitize(name)))
    }

    pub fn write(&mut self, slot: usize, bytes: &[u8], preview: &str) -> Result<(), String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            SaveStore::Dir(dir) => {
                std::fs::create_dir_all(&*dir).map_err(|e| e.to_string())?;
                std::fs::write(dir.join(format!("slot-{slot}.ksav")), bytes).map_err(|e| e.to_string())?;
                std::fs::write(dir.join(format!("slot-{slot}.txt")), preview).map_err(|e| e.to_string())
            }
            #[cfg(target_arch = "wasm32")]
            SaveStore::Local(prefix) => {
                let storage = local_storage().ok_or("localStorage is unavailable")?;
                storage
                    .set_item(&format!("{prefix}{slot}"), &to_hex(bytes))
                    .map_err(|_| "storage is full".to_string())?;
                storage
                    .set_item(&format!("{prefix}{slot}:preview"), preview)
                    .map_err(|_| "storage is full".to_string())?;
                storage
                    .set_item(&format!("{prefix}{slot}:saved_at"), &timestamp().to_string())
                    .map_err(|_| "storage is full".into())
            }
            SaveStore::Memory { slots, .. } => {
                slots.insert(slot, (bytes.to_vec(), preview.to_string(), timestamp()));
                Ok(())
            }
        }
    }

    pub fn read(&self, slot: usize) -> Option<Vec<u8>> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            SaveStore::Dir(dir) => std::fs::read(dir.join(format!("slot-{slot}.ksav"))).ok(),
            #[cfg(target_arch = "wasm32")]
            SaveStore::Local(prefix) => from_hex(&local_storage()?.get_item(&format!("{prefix}{slot}")).ok()??),
            SaveStore::Memory { slots, .. } => slots.get(&slot).map(|s| s.0.clone()),
        }
    }

    pub fn language(&self) -> Option<String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            SaveStore::Dir(dir) => std::fs::read_to_string(dir.join("language.txt")).ok(),
            #[cfg(target_arch = "wasm32")]
            SaveStore::Local(prefix) => local_storage()?.get_item(&format!("{prefix}language")).ok()?,
            SaveStore::Memory { language, .. } => language.clone(),
        }
    }

    pub fn set_language(&mut self, language: &str) -> Result<(), String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            SaveStore::Dir(dir) => {
                std::fs::create_dir_all(&*dir).map_err(|e| e.to_string())?;
                std::fs::write(dir.join("language.txt"), language).map_err(|e| e.to_string())
            }
            #[cfg(target_arch = "wasm32")]
            SaveStore::Local(prefix) => local_storage()
                .ok_or("localStorage is unavailable")?
                .set_item(&format!("{prefix}language"), language)
                .map_err(|_| "storage is full".into()),
            SaveStore::Memory { language: saved, .. } => {
                *saved = Some(language.to_string());
                Ok(())
            }
        }
    }

    /// Data shared by every playthrough; see [`kairos_vm::Persistent`].
    pub fn persistent(&self) -> Option<Vec<u8>> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            SaveStore::Dir(dir) => std::fs::read(dir.join("persistent.kper")).ok(),
            #[cfg(target_arch = "wasm32")]
            SaveStore::Local(prefix) => from_hex(&local_storage()?.get_item(&format!("{prefix}persistent")).ok()??),
            SaveStore::Memory { persistent, .. } => persistent.clone(),
        }
    }

    pub fn write_persistent(&mut self, bytes: &[u8]) -> Result<(), String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            SaveStore::Dir(dir) => {
                std::fs::create_dir_all(&*dir).map_err(|e| e.to_string())?;
                std::fs::write(dir.join("persistent.kper"), bytes).map_err(|e| e.to_string())
            }
            #[cfg(target_arch = "wasm32")]
            SaveStore::Local(prefix) => local_storage()
                .ok_or("localStorage is unavailable")?
                .set_item(&format!("{prefix}persistent"), &to_hex(bytes))
                .map_err(|_| "storage is full".into()),
            SaveStore::Memory { persistent, .. } => {
                *persistent = Some(bytes.to_vec());
                Ok(())
            }
        }
    }

    pub fn newest(&self) -> Option<usize> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            SaveStore::Dir(dir) => std::fs::read_dir(dir)
                .ok()?
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name();
                    let name = name.to_str()?;
                    let slot = name.strip_prefix("slot-")?.strip_suffix(".ksav")?.parse::<usize>().ok()?;
                    let modified =
                        entry.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
                    Some((slot, modified))
                })
                .max_by_key(|(_, modified)| *modified)
                .map(|(slot, _)| slot),
            #[cfg(target_arch = "wasm32")]
            SaveStore::Local(prefix) => {
                let storage = local_storage()?;
                (0..crate::view::SLOTS)
                    .filter_map(|slot| {
                        storage.get_item(&format!("{prefix}{slot}")).ok()??;
                        let saved_at = storage
                            .get_item(&format!("{prefix}{slot}:saved_at"))
                            .ok()
                            .flatten()
                            .and_then(|value| value.parse::<u128>().ok())
                            .unwrap_or(slot as u128);
                        Some((slot, saved_at))
                    })
                    .max_by_key(|(_, saved_at)| *saved_at)
                    .map(|(slot, _)| slot)
            }
            SaveStore::Memory { slots, .. } => {
                slots.iter().max_by_key(|(_, (_, _, timestamp))| timestamp).map(|(slot, _)| *slot)
            }
        }
    }

    pub fn preview(&self, slot: usize) -> Option<String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            SaveStore::Dir(dir) => dir
                .join(format!("slot-{slot}.ksav"))
                .exists()
                .then(|| std::fs::read_to_string(dir.join(format!("slot-{slot}.txt"))).unwrap_or_default()),
            #[cfg(target_arch = "wasm32")]
            SaveStore::Local(prefix) => {
                let storage = local_storage()?;
                storage.get_item(&format!("{prefix}{slot}")).ok()??;
                Some(storage.get_item(&format!("{prefix}{slot}:preview")).ok().flatten().unwrap_or_default())
            }
            SaveStore::Memory { slots, .. } => slots.get(&slot).map(|s| s.1.clone()),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn timestamp() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn timestamp() -> u128 {
    (js_sys::Date::now() * 1_000_000.0) as u128
}

#[cfg(target_arch = "wasm32")]
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(target_arch = "wasm32")]
fn from_hex(hex: &str) -> Option<Vec<u8>> {
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect()
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
    if s.is_empty() { "untitled".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_preference_is_separate_from_save_slots() {
        let mut store = SaveStore::Memory { slots: HashMap::new(), language: None, persistent: None };
        store.write(0, b"save", "preview").unwrap();
        assert_eq!(store.language(), None);
        store.set_language("ja").unwrap();
        assert_eq!(store.language().as_deref(), Some("ja"));
        assert_eq!(store.read(0).as_deref(), Some(&b"save"[..]));
    }

    #[test]
    fn project_names_become_safe_directory_names() {
        assert_eq!(sanitize("Hello Kairos"), "Hello_Kairos");
        assert_eq!(sanitize("../x"), "___x");
        assert_eq!(sanitize(""), "untitled");
    }

    #[test]
    fn newest_memory_slot_tracks_write_order() {
        let mut store = SaveStore::Memory { slots: HashMap::new(), language: None, persistent: None };
        store.write(4, b"first", "first").unwrap();
        store.write(1, b"newest", "newest").unwrap();
        assert_eq!(store.newest(), Some(1));
    }

    #[test]
    fn slots_round_trip_through_a_directory() {
        let dir = std::env::temp_dir().join(format!("kairos-saves-{}", std::process::id()));
        let mut store = SaveStore::Dir(dir.clone());
        assert_eq!(store.preview(0), None);
        store.write(0, b"KSAVdata", "Eileen: Hi.").unwrap();
        assert_eq!(store.read(0).as_deref(), Some(&b"KSAVdata"[..]));
        assert_eq!(store.preview(0).as_deref(), Some("Eileen: Hi."));
        assert_eq!(store.read(1), None);
        store.set_language("ja").unwrap();
        assert_eq!(store.persistent(), None);
        store.write_persistent(b"KPERdata").unwrap();
        let loaded = SaveStore::Dir(dir.clone());
        assert_eq!(loaded.language().as_deref(), Some("ja"));
        assert_eq!(loaded.persistent().as_deref(), Some(&b"KPERdata"[..]));
        assert_eq!(loaded.newest(), Some(0), "persistent data is not a save slot");
        assert_eq!(loaded.read(0).as_deref(), Some(&b"KSAVdata"[..]));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

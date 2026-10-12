//! Save slots: a VM save plus a one-line preview per slot.

use std::collections::HashMap;

pub enum SaveStore {
    /// `slot-N.ksav` and `slot-N.txt` files in a per-project directory.
    #[cfg(not(target_arch = "wasm32"))]
    Dir(std::path::PathBuf),
    /// `localStorage` entries under a per-project prefix.
    #[cfg(target_arch = "wasm32")]
    Local(String),
    Memory(HashMap<usize, (Vec<u8>, String, u128)>),
}

impl SaveStore {
    /// The platform data directory, e.g. `~/Library/Application Support/Kairos/saves/<project>`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn for_project(name: &str) -> SaveStore {
        match directories::ProjectDirs::from("", "", "Kairos") {
            Some(dirs) => SaveStore::Dir(dirs.data_dir().join("saves").join(sanitize(name))),
            None => SaveStore::Memory(HashMap::new()),
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
                let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                storage.set_item(&format!("{prefix}{slot}"), &hex).map_err(|_| "storage is full".to_string())?;
                storage
                    .set_item(&format!("{prefix}{slot}:preview"), preview)
                    .map_err(|_| "storage is full".to_string())?;
                storage
                    .set_item(&format!("{prefix}{slot}:saved_at"), &timestamp().to_string())
                    .map_err(|_| "storage is full".into())
            }
            SaveStore::Memory(slots) => {
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
            SaveStore::Local(prefix) => {
                let hex = local_storage()?.get_item(&format!("{prefix}{slot}")).ok()??;
                (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect()
            }
            SaveStore::Memory(slots) => slots.get(&slot).map(|s| s.0.clone()),
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
            SaveStore::Memory(slots) => {
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
            SaveStore::Memory(slots) => slots.get(&slot).map(|s| s.1.clone()),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn timestamp() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
}

#[cfg(target_arch = "wasm32")]
fn timestamp() -> u128 {
    (js_sys::Date::now() * 1_000_000.0) as u128
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
    fn project_names_become_safe_directory_names() {
        assert_eq!(sanitize("Hello Kairos"), "Hello_Kairos");
        assert_eq!(sanitize("../x"), "___x");
        assert_eq!(sanitize(""), "untitled");
    }

    #[test]
    fn newest_memory_slot_tracks_write_order() {
        let mut store = SaveStore::Memory(HashMap::new());
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
        std::fs::remove_dir_all(dir).unwrap();
    }
}

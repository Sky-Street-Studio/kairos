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
        slots: HashMap<usize, (Vec<u8>, String)>,
        language: Option<String>,
    },
}

impl SaveStore {
    /// The platform data directory, e.g. `~/Library/Application Support/Kairos/saves/<project>`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn for_project(name: &str) -> SaveStore {
        match directories::ProjectDirs::from("", "", "Kairos") {
            Some(dirs) => SaveStore::Dir(dirs.data_dir().join("saves").join(sanitize(name))),
            None => SaveStore::Memory { slots: HashMap::new(), language: None },
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
                storage.set_item(&format!("{prefix}{slot}"), &hex).map_err(|_| "storage is full")?;
                storage.set_item(&format!("{prefix}{slot}:preview"), preview).map_err(|_| "storage is full".into())
            }
            SaveStore::Memory { slots, .. } => {
                slots.insert(slot, (bytes.to_vec(), preview.to_string()));
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
        let mut store = SaveStore::Memory { slots: HashMap::new(), language: None };
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
    fn slots_round_trip_through_a_directory() {
        let dir = std::env::temp_dir().join(format!("kairos-saves-{}", std::process::id()));
        let mut store = SaveStore::Dir(dir.clone());
        assert_eq!(store.preview(0), None);
        store.write(0, b"KSAVdata", "Eileen: Hi.").unwrap();
        assert_eq!(store.read(0).as_deref(), Some(&b"KSAVdata"[..]));
        assert_eq!(store.preview(0).as_deref(), Some("Eileen: Hi."));
        assert_eq!(store.read(1), None);
        store.set_language("ja").unwrap();
        let loaded = SaveStore::Dir(dir.clone());
        assert_eq!(loaded.language().as_deref(), Some("ja"));
        assert_eq!(loaded.read(0).as_deref(), Some(&b"KSAVdata"[..]));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

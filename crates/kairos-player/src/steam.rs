use std::path::{Path, PathBuf};

use kairos_project::SteamConfig;
use kairos_vm::{Persistent, Value};
use steamworks::Client;

pub(crate) struct Steam {
    client: Client,
    achievements: Vec<(String, String)>,
    save_dir: PathBuf,
    cloud_prefix: String,
    persistent: Option<Vec<u8>>,
    pending_stats: bool,
}

impl Steam {
    pub(crate) fn new(config: Option<&SteamConfig>, project_name: &str) -> Option<Self> {
        let config = config?;
        let save_dir = crate::saves::SaveStore::directory_for_project(project_name)?;
        let client = match Client::init_app(config.app_id) {
            Ok(client) => client,
            Err(error) => {
                eprintln!("warning: Steam is unavailable: {error}");
                return None;
            }
        };
        client.user_stats().request_user_stats(client.user().steam_id().raw());
        let steam = Self {
            client,
            achievements: config
                .achievements
                .iter()
                .map(|(achievement, key)| (achievement.clone(), key.clone()))
                .collect(),
            save_dir,
            cloud_prefix: format!("Kairos/{}/", sanitize(project_name)),
            persistent: None,
            pending_stats: false,
        };
        steam.sync_cloud();
        Some(steam)
    }

    pub(crate) fn update(&mut self) {
        self.client.run_callbacks();
        let Ok(bytes) = std::fs::read(self.save_dir.join("persistent.kper")) else { return };
        if self.persistent.as_ref() == Some(&bytes) && !self.pending_stats {
            return;
        }
        let Ok(persistent) = Persistent::from_bytes(&bytes) else { return };
        let stats = self.client.user_stats();
        let mut changed = false;
        let mut waiting_for_stats = false;
        for (achievement, key) in &self.achievements {
            if !persistent.values.get(key).is_some_and(is_unlocked) {
                continue;
            }
            match stats.achievement(achievement).get() {
                Ok(true) => {}
                Ok(false) => changed |= stats.achievement(achievement).set().is_ok(),
                Err(()) => waiting_for_stats = true,
            }
        }
        if changed || self.pending_stats {
            self.pending_stats = stats.store_stats().is_err();
        }
        if !waiting_for_stats {
            self.persistent = Some(bytes);
        }
    }

    pub(crate) fn sync_cloud(&self) {
        let remote = self.client.remote_storage();
        if !remote.is_cloud_enabled_for_app() || !remote.is_cloud_enabled_for_account() {
            return;
        }
        for entry in remote.files() {
            let Ok(relative) = entry.name.strip_prefix(&self.cloud_prefix).ok_or(()) else { continue };
            let relative = Path::new(relative);
            if relative.components().any(|part| !matches!(part, std::path::Component::Normal(_))) {
                continue;
            }
            if crosses_symlink(&self.save_dir, relative) {
                continue;
            }
            let path = self.save_dir.join(relative);
            let remote_file = remote.file(&entry.name);
            let remote_time = remote_file.timestamp();
            let local_time = path
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|time| time.as_secs() as i64);
            if local_time.is_some_and(|time| time >= remote_time) {
                continue;
            }
            if let Some(parent) = path.parent()
                && std::fs::create_dir_all(parent).is_err()
            {
                continue;
            }
            let mut reader = remote_file.read();
            if let Err(error) =
                std::fs::File::create(&path).and_then(|mut file| std::io::copy(&mut reader, &mut file).map(|_| ()))
            {
                eprintln!("warning: could not download Steam save {}: {error}", path.display());
            }
        }
        for path in files_under(&self.save_dir) {
            let Ok(relative) = path.strip_prefix(&self.save_dir) else { continue };
            let name = format!("{}{}", self.cloud_prefix, relative.to_string_lossy().replace('\\', "/"));
            let local_time = path
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|time| time.as_secs() as i64)
                .unwrap_or_default();
            if remote.file(&name).exists() && remote.file(&name).timestamp() >= local_time {
                continue;
            }
            let Ok(mut source) = std::fs::File::open(&path) else { continue };
            let mut writer = remote.file(&name).write();
            if let Err(error) = std::io::copy(&mut source, &mut writer) {
                eprintln!("warning: could not upload Steam save {}: {error}", path.display());
            }
        }
    }
}

fn sanitize(name: &str) -> String {
    let name: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
    if name.is_empty() { "untitled".into() } else { name }
}

fn is_unlocked(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Int(value) => *value != 0,
        Value::Float(value) => *value != 0.0,
        Value::Str(value) => !value.is_empty(),
        Value::None | Value::List(_) | Value::Dict(_) => false,
    }
}

fn files_under(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .flat_map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => files_under(&entry.path()),
            Ok(kind) if kind.is_file() => vec![entry.path()],
            _ => Vec::new(),
        })
        .collect()
}

fn crosses_symlink(base: &Path, relative: &Path) -> bool {
    let mut path = base.to_path_buf();
    relative.components().any(|part| {
        path.push(part);
        path.symlink_metadata().is_ok_and(|meta| meta.file_type().is_symlink())
    })
}

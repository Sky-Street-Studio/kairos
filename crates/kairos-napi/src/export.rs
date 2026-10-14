use std::path::{Path, PathBuf};

use kairos_project::export::{self, ExportTarget};

pub fn export_game(
    project: &str,
    target: &str,
    output: &str,
    player_binary: Option<&str>,
    player_pkg: Option<&str>,
    pack_assets: bool,
) -> Result<String, String> {
    let target = match target {
        "web" => ExportTarget::Web,
        "desktop" => ExportTarget::Desktop,
        "windows-x64" => ExportTarget::WindowsX64,
        "linux-x64" => ExportTarget::LinuxX64,
        "macos" => ExportTarget::MacOS,
        _ => {
            return Err(format!(
                "unknown export target `{target}`; expected `web`, `desktop`, `windows-x64`, `linux-x64` or `macos`"
            ));
        }
    };
    let player_binary = player_binary.map(Path::new);
    let player_pkg = player_pkg.map(Path::new);
    export::export(Path::new(project), Path::new(output), target, player_binary, player_pkg, pack_assets)
        .map(|path: PathBuf| path.to_string_lossy().into_owned())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_export_reports_when_player_binary_is_missing() {
        let root = std::env::temp_dir().join(format!("kairos-napi-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::write(root.join("kairos.toml"), "name = 'test'").unwrap();
        std::fs::write(root.join("script/start.kai"), "label start:\n    return\n").unwrap();
        let error = export_game(root.to_str().unwrap(), "desktop", "out", None, None, false).unwrap_err();
        assert!(error.contains("desktop export requires an explicit Kairos player binary path"));
        let _ = std::fs::remove_dir_all(root);
    }
}

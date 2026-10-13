use std::path::{Path, PathBuf};

use crate::{PlayerProjectJson, PlayerScriptJson, PlayerTranslationJson, Project, ProjectError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    Web,
    Desktop,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error("project has check errors:\n{0}")]
    Check(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Player(String),
    #[error("failed to serialize game.json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Checks a project and assembles a web folder or a host-platform desktop bundle.
pub fn export(
    root: &Path,
    output: &Path,
    target: ExportTarget,
    player_binary: Option<&Path>,
    player_pkg: Option<&Path>,
) -> Result<PathBuf, ExportError> {
    let project = Project::load(root)?;
    let mut errors = project.diagnostics_report();
    for (path, message) in project.asset_diagnostics()? {
        use std::fmt::Write as _;
        writeln!(errors, "{}: {message}", path.display()).unwrap();
    }
    if !errors.is_empty() {
        return Err(ExportError::Check(errors));
    }

    match target {
        ExportTarget::Web => export_web(root, output, &project, player_pkg),
        ExportTarget::Desktop => export_desktop(root, output, &project, player_binary),
    }
}

fn export_web(
    root: &Path,
    output: &Path,
    project: &Project,
    player_pkg: Option<&Path>,
) -> Result<PathBuf, ExportError> {
    let default_pkg = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kairos-player/pkg");
    let pkg = player_pkg.unwrap_or(&default_pkg);
    if !pkg.is_dir() {
        return Err(ExportError::Player(format!(
            "Web player package not found at {}; build it with scripts/build-wasm.sh or pass --player-pkg",
            pkg.display()
        )));
    }
    std::fs::create_dir_all(output)?;
    copy_directory(pkg, output)?;
    copy_project_files(root, output)?;

    let scripts = project
        .files
        .iter()
        .map(|path| Ok(PlayerScriptJson { path: relative_path(root, path)?, source: std::fs::read_to_string(path)? }))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    let translations = std::fs::read_dir(root.join("tl"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "toml"))
        .map(|entry| {
            Ok(PlayerTranslationJson {
                lang: entry.path().file_stem().and_then(|name| name.to_str()).unwrap_or_default().to_string(),
                source: std::fs::read_to_string(entry.path())?,
            })
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    let assets =
        crate::assets::collect_paths(&root.join("assets"))?.into_iter().map(|path| format!("assets/{path}")).collect();
    let game = PlayerProjectJson {
        manifest: std::fs::read_to_string(root.join("kairos.toml"))?,
        scripts,
        translations,
        asset_base_url: "./assets/".into(),
        assets,
        theme: read_optional(root, "ui/theme.toml")?,
        title: read_optional(root, "ui/title.toml")?,
    };
    std::fs::write(output.join("game.json"), serde_json::to_vec_pretty(&game)?)?;
    std::fs::write(output.join("index.html"), WEB_INDEX)?;
    Ok(output.to_path_buf())
}

fn export_desktop(
    root: &Path,
    output: &Path,
    project: &Project,
    player_binary: Option<&Path>,
) -> Result<PathBuf, ExportError> {
    let binary = player_binary
        .ok_or_else(|| ExportError::Player("desktop export requires an explicit Kairos player binary path".into()))?;
    if !binary.is_file() {
        return Err(ExportError::Player(format!("Kairos player binary not found at {}", binary.display())));
    }

    #[cfg(target_os = "macos")]
    let bundle = {
        let name = safe_name(&project.manifest.name);
        let app = output.join(format!("{}.app", bundle_name(&project.manifest.name)));
        let executable = app.join("Contents/MacOS").join(&name);
        let game = app.join("Contents/Resources/game");
        std::fs::create_dir_all(executable.parent().unwrap())?;
        std::fs::create_dir_all(&game)?;
        std::fs::copy(binary, &executable)?;
        copy_project_files(root, &game)?;
        std::fs::write(
            app.join("Contents/Info.plist"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleName</key><string>{}</string><key>CFBundleExecutable</key><string>{name}</string><key>CFBundleIdentifier</key><string>studio.skystreet.kairos.game</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n",
                xml_escape(&project.manifest.name)
            ),
        )?;
        app
    };
    #[cfg(not(target_os = "macos"))]
    let bundle = {
        let name = safe_name(&project.manifest.name);
        #[cfg(target_os = "windows")]
        let executable = output.join(format!("{name}.exe"));
        #[cfg(not(target_os = "windows"))]
        let executable = output.join(name);
        let game = output.join("game");
        std::fs::create_dir_all(output)?;
        std::fs::copy(binary, &executable)?;
        copy_project_files(root, &game)?;
        executable
    };
    #[cfg(target_os = "macos")]
    return Ok(bundle);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle;
        Ok(output.to_path_buf())
    }
}

fn copy_project_files(root: &Path, destination: &Path) -> Result<(), std::io::Error> {
    for name in ["kairos.toml", "script", "assets", "ui", "tl"] {
        let source = root.join(name);
        if !source.exists() {
            continue;
        }
        let dest = destination.join(name);
        if source.is_dir() {
            copy_directory(&source, &dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(source, dest)?;
        }
    }
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&source, &destination)?;
        } else if entry.file_type()?.is_file() {
            std::fs::copy(source, destination)?;
        }
    }
    Ok(())
}

fn read_optional(root: &Path, rel: &str) -> Result<Option<String>, std::io::Error> {
    match std::fs::read_to_string(root.join(rel)) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn relative_path(root: &Path, path: &Path) -> Result<String, std::io::Error> {
    path.strip_prefix(root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
}

fn safe_name(name: &str) -> String {
    let name: String =
        name.chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' || character == '_' { character } else { '-' }
            })
            .collect();
    let name = name.trim_matches('-').to_ascii_lowercase();
    if name.is_empty() { "kairos-game".into() } else { name }
}

#[cfg(target_os = "macos")]
fn bundle_name(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == ' ' || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect();
    let name = name.trim_matches([' ', '-']);
    if name.is_empty() { "Kairos Game".into() } else { name.into() }
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

const WEB_INDEX: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Kairos game</title><style>html,body{margin:0;width:100%;height:100%;overflow:hidden;background:#111}canvas{width:100%;height:100%;display:block}</style></head><body><canvas id=\"game\"></canvas><script type=\"module\">import init,{WebPlayer} from './kairos_player.js';await init();const response=await fetch('./game.json');if(!response.ok)throw new Error('Could not load game.json');const canvas=document.querySelector('#game');const player=new WebPlayer(canvas);await player.ready();player.resize(window.innerWidth,window.innerHeight);window.addEventListener('resize',()=>player.resize(window.innerWidth,window.innerHeight));player.load(JSON.stringify(await response.json()));player.start();</script></body></html>\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_export_copies_the_project_and_host_player() {
        let root = std::env::temp_dir().join(format!("kairos-desktop-export-{}", std::process::id()));
        let output = root.with_extension("out");
        let binary = root.with_extension("bin");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&output);
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("kairos.toml"), "name = 'Demo Game'").unwrap();
        std::fs::write(root.join("script/start.kai"), "label start:\n    return\n").unwrap();
        std::fs::write(root.join("assets/image.png"), []).unwrap();
        std::fs::write(&binary, "player binary").unwrap();

        export(&root, &output, ExportTarget::Desktop, Some(&binary), None).unwrap();
        #[cfg(target_os = "macos")]
        let (executable, game) = (
            output.join("Demo Game.app/Contents/MacOS/demo-game"),
            output.join("Demo Game.app/Contents/Resources/game"),
        );
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        let (executable, game) = (output.join("demo-game"), output.join("game"));
        #[cfg(target_os = "windows")]
        let (executable, game) = (output.join("demo-game.exe"), output.join("game"));
        assert_eq!(std::fs::read_to_string(executable).unwrap(), "player binary");
        assert_eq!(std::fs::read_to_string(game.join("script/start.kai")).unwrap(), "label start:\n    return\n");
        assert!(game.join("assets/image.png").is_file());
        #[cfg(target_os = "macos")]
        assert!(output.join("Demo Game.app/Contents/Info.plist").is_file());

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(output);
        let _ = std::fs::remove_file(binary);
    }

    #[test]
    fn web_export_game_json_uses_the_player_load_shape_and_rejects_check_errors() {
        let root = std::env::temp_dir().join(format!("kairos-export-{}", std::process::id()));
        let output = root.with_extension("out");
        let pkg = root.with_extension("pkg");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&output);
        let _ = std::fs::remove_dir_all(&pkg);
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::create_dir_all(root.join("assets/images")).unwrap();
        std::fs::create_dir_all(root.join("ui")).unwrap();
        std::fs::create_dir_all(root.join("tl")).unwrap();
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(root.join("kairos.toml"), "name = 'test'").unwrap();
        std::fs::write(root.join("script/start.kai"), "label start:\n    return\n").unwrap();
        std::fs::write(root.join("ui/theme.toml"), "").unwrap();
        std::fs::write(root.join("tl/fr.toml"), "hello = 'bonjour'").unwrap();
        std::fs::write(root.join("assets/images/bg.png"), []).unwrap();
        std::fs::write(pkg.join("kairos_player.js"), "export class WebPlayer {}").unwrap();
        std::fs::write(pkg.join("kairos_player_bg.wasm"), []).unwrap();

        export(&root, &output, ExportTarget::Web, None, Some(&pkg)).unwrap();
        let json: PlayerProjectJson =
            serde_json::from_slice(&std::fs::read(output.join("game.json")).unwrap()).unwrap();
        assert_eq!(json.manifest, "name = 'test'");
        assert_eq!(json.scripts[0].path, "script/start.kai");
        assert_eq!(json.translations[0].lang, "fr");
        assert_eq!(json.asset_base_url, "./assets/");
        assert_eq!(json.assets[0], "assets/images/bg.png");
        assert_eq!(json.theme.as_deref(), Some(""));
        assert!(output.join("kairos_player.js").is_file());
        assert_eq!(std::fs::read_to_string(output.join("script/start.kai")).unwrap(), "label start:\n    return\n");

        std::fs::write(root.join("script/start.kai"), "label start:\n    jump missing\n").unwrap();
        let error = export(&root, &output, ExportTarget::Web, None, Some(&pkg)).unwrap_err();
        assert!(error.to_string().contains("unknown label `missing`"));

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(output);
        let _ = std::fs::remove_dir_all(pkg);
    }
}

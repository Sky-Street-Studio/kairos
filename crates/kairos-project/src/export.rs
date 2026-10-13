use std::path::{Path, PathBuf};

use crate::{PlayerProjectJson, PlayerScriptJson, PlayerTranslationJson, Project, ProjectError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    Web,
    Desktop,
    WindowsX64,
    LinuxX64,
    MacOS,
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
    #[error("{0}")]
    Output(String),
    #[error("failed to serialize game.json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Checks a project and assembles a web folder or a desktop bundle.
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

    reject_overlapping_output(root, output)?;
    match target {
        ExportTarget::Web => export_web(root, output, &project, player_pkg),
        target => export_desktop(root, output, &project, player_binary, target),
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
    let index = WEB_INDEX
        .replace("<title>Kairos game</title>", &format!("<title>{}</title>", xml_escape(&project.manifest.name)));
    std::fs::write(output.join("index.html"), index)?;
    Ok(output.to_path_buf())
}

fn export_desktop(
    root: &Path,
    output: &Path,
    project: &Project,
    player_binary: Option<&Path>,
    target: ExportTarget,
) -> Result<PathBuf, ExportError> {
    let target = match target {
        ExportTarget::Desktop => host_target(),
        target => target,
    };
    if !target_is_host(target) && player_binary.is_none() {
        return Err(ExportError::Player(format!(
            "cross-target export for {} requires a matching player binary; Kairos can only build for the host platform",
            target_name(target)
        )));
    }
    let binary = player_binary
        .ok_or_else(|| ExportError::Player("desktop export requires an explicit Kairos player binary path".into()))?;
    if !binary.is_file() {
        return Err(ExportError::Player(format!("Kairos player binary not found at {}", binary.display())));
    }

    if target == ExportTarget::MacOS {
        let app = export_macos(root, output, project, binary)?;
        sign_macos(&app)?;
        return Ok(app);
    }
    let name = safe_name(&project.manifest.name);
    let game = output.join("game");
    std::fs::create_dir_all(output)?;
    let (executable, launcher) = match target {
        ExportTarget::WindowsX64 => (output.join(format!("{name}.exe")), output.join("run.bat")),
        ExportTarget::LinuxX64 => (output.join(&name), output.join("run.sh")),
        _ => return Err(ExportError::Player(format!("unsupported desktop target {}", target_name(target)))),
    };
    std::fs::copy(binary, &executable)?;
    copy_project_files(root, &game)?;
    let launcher_text = if target == ExportTarget::WindowsX64 {
        format!("@echo off\r\ncd /d \"%~dp0\"\r\n\"%~dp0\\{name}.exe\"\r\n")
    } else {
        format!("#!/bin/sh\ncd \"$(dirname \"$0\")\"\nexec \"./{name}\"\n")
    };
    std::fs::write(&launcher, launcher_text)?;
    #[cfg(unix)]
    if target == ExportTarget::LinuxX64 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(output.to_path_buf())
}

fn export_macos(root: &Path, output: &Path, project: &Project, binary: &Path) -> Result<PathBuf, ExportError> {
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
    Ok(app)
}

fn host_target() -> ExportTarget {
    #[cfg(target_os = "macos")]
    return ExportTarget::MacOS;
    #[cfg(target_os = "windows")]
    return ExportTarget::WindowsX64;
    #[cfg(target_os = "linux")]
    return ExportTarget::LinuxX64;
    #[allow(unreachable_code)]
    ExportTarget::Desktop
}

fn target_is_host(target: ExportTarget) -> bool {
    target == host_target()
        && (target != ExportTarget::WindowsX64 && target != ExportTarget::LinuxX64 || cfg!(target_arch = "x86_64"))
}

fn target_name(target: ExportTarget) -> &'static str {
    match target {
        ExportTarget::Web => "web",
        ExportTarget::Desktop => "host desktop",
        ExportTarget::WindowsX64 => "windows-x64",
        ExportTarget::LinuxX64 => "linux-x64",
        ExportTarget::MacOS => "macos",
    }
}

fn sign_macos(app: &Path) -> Result<(), ExportError> {
    let identity = std::env::var("KAIROS_MACOS_SIGNING_IDENTITY").ok();
    let team = std::env::var("KAIROS_MACOS_TEAM_ID").ok();
    let profile = std::env::var("KAIROS_MACOS_NOTARY_PROFILE").ok();
    if identity.is_none() && team.is_none() && profile.is_none() {
        eprintln!(
            "warning: macOS app is unsigned; set KAIROS_MACOS_SIGNING_IDENTITY, KAIROS_MACOS_TEAM_ID and KAIROS_MACOS_NOTARY_PROFILE to sign and notarize"
        );
        return Ok(());
    }
    let (Some(identity), Some(team), Some(profile)) = (identity, team, profile) else {
        return Err(ExportError::Player("macOS signing requires KAIROS_MACOS_SIGNING_IDENTITY, KAIROS_MACOS_TEAM_ID and KAIROS_MACOS_NOTARY_PROFILE".into()));
    };
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let status = Command::new("codesign")
            .args(["--force", "--deep", "--options", "runtime", "--timestamp", "--sign", &identity])
            .arg(app)
            .status()?;
        if !status.success() {
            return Err(ExportError::Player("codesign failed".into()));
        }
        let status = Command::new("xcrun")
            .args(["notarytool", "submit"])
            .arg(app)
            .args(["--keychain-profile", &profile, "--team-id", &team, "--wait"])
            .status()?;
        if !status.success() {
            return Err(ExportError::Player("notarytool failed".into()));
        }
        let status = Command::new("xcrun").args(["stapler", "staple"]).arg(app).status()?;
        if !status.success() {
            return Err(ExportError::Player("stapler failed".into()));
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (identity, team, profile, app);
        eprintln!("warning: macOS signing was skipped because export is running on a non-macOS host");
    }
    Ok(())
}

const PROJECT_ENTRIES: [&str; 5] = ["kairos.toml", "script", "assets", "ui", "tl"];

/// Exporting into the project root or a copied folder would overwrite sources or copy the export
/// into itself forever.
fn reject_overlapping_output(root: &Path, output: &Path) -> Result<(), ExportError> {
    let root = root.canonicalize()?;
    let output = resolve_existing_prefix(output)?;
    if output == root || PROJECT_ENTRIES.iter().any(|entry| output.starts_with(root.join(entry))) {
        return Err(ExportError::Output(format!(
            "choose an output folder outside the project root and its {} folders",
            PROJECT_ENTRIES[1..].join(", ")
        )));
    }
    Ok(())
}

fn resolve_existing_prefix(path: &Path) -> Result<PathBuf, std::io::Error> {
    let path = std::path::absolute(path)?;
    let mut existing = path.as_path();
    let mut rest = Vec::new();
    while !existing.exists() {
        let Some(parent) = existing.parent() else { break };
        rest.push(existing.file_name().unwrap_or_default().to_owned());
        existing = parent;
    }
    let mut resolved = existing.canonicalize()?;
    resolved.extend(rest.iter().rev());
    Ok(resolved)
}

fn copy_project_files(root: &Path, destination: &Path) -> Result<(), std::io::Error> {
    for name in PROJECT_ENTRIES {
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
    fn platform_export_adds_a_launcher_and_bundles_project_files() {
        let root = std::env::temp_dir().join(format!("kairos-platform-export-{}", std::process::id()));
        let binary = root.with_extension("bin");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::write(root.join("kairos.toml"), "name = 'Demo Game'").unwrap();
        std::fs::write(root.join("script/start.kai"), "label start:\n    return\n").unwrap();
        std::fs::write(&binary, "player binary").unwrap();

        for (target, executable, launcher) in
            [(ExportTarget::WindowsX64, "demo-game.exe", "run.bat"), (ExportTarget::LinuxX64, "demo-game", "run.sh")]
        {
            let output = root.with_extension(target_name(target));
            let _ = std::fs::remove_dir_all(&output);
            export(&root, &output, target, Some(&binary), None).unwrap();
            assert_eq!(std::fs::read_to_string(output.join(executable)).unwrap(), "player binary");
            assert!(output.join(launcher).is_file());
            assert!(output.join("game/script/start.kai").is_file());
            let _ = std::fs::remove_dir_all(output);
        }
        let output = root.with_extension("macos");
        let _ = std::fs::remove_dir_all(&output);
        let app = export_macos(&root, &output, &Project::load(&root).unwrap(), &binary).unwrap();
        assert!(app.join("Contents/MacOS/demo-game").is_file());
        assert!(app.join("Contents/Resources/game/script/start.kai").is_file());
        assert!(app.join("Contents/Info.plist").is_file());
        let _ = std::fs::remove_dir_all(output);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_file(binary);
    }

    #[test]
    fn cross_target_export_without_a_target_player_explains_the_host_limit() {
        let root = std::env::temp_dir().join(format!("kairos-cross-export-{}", std::process::id()));
        let output = root.with_extension("out");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&output);
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::write(root.join("kairos.toml"), "name = 'test'").unwrap();
        std::fs::write(root.join("script/start.kai"), "label start:\n    return\n").unwrap();
        let target =
            if host_target() == ExportTarget::WindowsX64 { ExportTarget::LinuxX64 } else { ExportTarget::WindowsX64 };
        let error = export(&root, &output, target, None, None).unwrap_err();
        assert!(error.to_string().contains("requires a matching player binary"));
        let _ = std::fs::remove_dir_all(root);
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
        assert!(std::fs::read_to_string(output.join("index.html")).unwrap().contains("<title>test</title>"));
        assert_eq!(std::fs::read_to_string(output.join("script/start.kai")).unwrap(), "label start:\n    return\n");

        for inside in [root.clone(), root.join("assets/export"), root.join("script")] {
            let error = export(&root, &inside, ExportTarget::Web, None, Some(&pkg)).unwrap_err();
            assert!(matches!(error, ExportError::Output(_)), "{inside:?} should be rejected");
        }
        assert!(!root.join("assets/export").exists());
        export(&root, &root.join("build"), ExportTarget::Web, None, Some(&pkg)).unwrap();

        std::fs::write(root.join("script/start.kai"), "label start:\n    jump missing\n").unwrap();
        let error = export(&root, &output, ExportTarget::Web, None, Some(&pkg)).unwrap_err();
        assert!(error.to_string().contains("unknown label `missing`"));

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(output);
        let _ = std::fs::remove_dir_all(pkg);
    }
}

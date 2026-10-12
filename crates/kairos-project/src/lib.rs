//! Project manifests, script loading and the asset naming rule shared by the CLI, player and editor.

pub mod assets;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kairos_script::Diagnostic;
use kairos_vm::Program;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default = "default_start")]
    pub start: String,
    #[serde(default)]
    pub window: Window,
    #[serde(default)]
    pub audio: AudioSettings,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub duck: f32,
    pub duck_fade: f64,
}

impl Default for AudioSettings {
    fn default() -> Self {
        AudioSettings { duck: 1.0, duck_fade: 0.25 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct AudioLoop {
    pub loop_start: f64,
    pub loop_end: Option<f64>,
}

pub type AudioMetadata = std::collections::HashMap<String, AudioLoop>;

pub fn parse_audio_metadata(text: &str) -> Result<AudioMetadata, toml::de::Error> {
    toml::from_str(text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Window {
    pub width: u32,
    pub height: u32,
}

impl Default for Window {
    fn default() -> Self {
        Window { width: 1280, height: 720 }
    }
}

fn default_start() -> String {
    "start".into()
}

pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub program: Arc<Program>,
    /// Script paths in compile order; [`Program::files`] indexes into this list.
    pub files: Vec<PathBuf>,
    /// Diagnostics paired with the file they came from.
    pub diagnostics: Vec<(PathBuf, Diagnostic)>,
    pub audio_metadata: AudioMetadata,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("kairos.toml: {0}")]
    Manifest(#[from] toml::de::Error),
    #[error("assets/audio/audio.toml: {0}")]
    AudioMetadata(#[source] toml::de::Error),
}

impl Project {
    /// Loads `kairos.toml` and every `script/*.kai` under `root`.
    pub fn load(root: &Path) -> Result<Project, ProjectError> {
        let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| ProjectError::Io(p.to_path_buf(), e));
        let manifest = read(&root.join("kairos.toml"))?;
        let script_dir = root.join("script");
        let paths: Vec<PathBuf> = std::fs::read_dir(&script_dir)
            .map_err(|e| ProjectError::Io(script_dir.clone(), e))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "kai"))
            .collect();
        let sources = paths.into_iter().map(|p| Ok((read(&p)?, p))).collect::<Result<Vec<_>, ProjectError>>()?;
        let mut project = Project::from_sources(&manifest, sources.into_iter().map(|(text, path)| (path, text)))?;
        project.root = root.to_path_buf();
        let metadata_path = root.join("assets/audio/audio.toml");
        project.audio_metadata = match std::fs::read_to_string(&metadata_path) {
            Ok(text) => parse_audio_metadata(&text).map_err(ProjectError::AudioMetadata)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AudioMetadata::new(),
            Err(e) => return Err(ProjectError::Io(metadata_path, e)),
        };
        Ok(project)
    }

    /// Builds a project from in-memory sources, compiled in path order. `root` is left empty.
    pub fn from_sources(
        manifest: &str,
        sources: impl IntoIterator<Item = (PathBuf, String)>,
    ) -> Result<Project, ProjectError> {
        let manifest: Manifest = toml::from_str(manifest)?;
        let mut sources: Vec<_> = sources.into_iter().collect();
        sources.sort_by(|a, b| a.0.cmp(&b.0));

        let mut diagnostics = Vec::new();
        let mut parsed = Vec::new();
        for (path, text) in &sources {
            let (stmts, diags) = kairos_script::parse(text);
            diagnostics.extend(diags.into_iter().map(|d| (path.clone(), d)));
            parsed.push(stmts);
        }
        let (program, diags) = kairos_vm::compile(&parsed);
        diagnostics.extend(diags.into_iter().map(|(i, d)| (sources[i].0.clone(), d)));
        let files = sources.into_iter().map(|(p, _)| p).collect();
        Ok(Project {
            root: PathBuf::new(),
            manifest,
            program: Arc::new(program),
            files,
            diagnostics,
            audio_metadata: AudioMetadata::new(),
        })
    }

    /// One `path:line: message` line per diagnostic.
    pub fn diagnostics_report(&self) -> String {
        let mut out = String::new();
        for (file, d) in &self.diagnostics {
            writeln!(out, "{}:{}: {}", file.display(), d.line, d.message).unwrap();
        }
        out
    }

    /// The first instruction at or after `line` in the script at `path`.
    pub fn pc_at(&self, path: &Path, line: usize) -> Option<usize> {
        let file = self.files.iter().position(|f| f == path)?;
        self.program.pc_at(file, line)
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join(assets::ASSETS_DIR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_defaults_to_720p_and_reads_overrides() {
        let p = Project::from_sources("name = \"x\"", []).unwrap();
        assert_eq!(p.manifest.window, Window { width: 1280, height: 720 });
        assert_eq!(p.manifest.audio.duck, 1.0);
        assert_eq!(p.manifest.audio.duck_fade, 0.25);
        let p = Project::from_sources("name = \"x\"\n[window]\nwidth = 800", []).unwrap();
        assert_eq!(p.manifest.window, Window { width: 800, height: 720 });
    }

    #[test]
    fn audio_metadata_is_keyed_by_audio_relative_path() {
        let metadata = parse_audio_metadata("[\"music/theme.ogg\"]\nloop_start = 12.48\nloop_end = 96.0\n").unwrap();
        assert_eq!(metadata.get("music/theme.ogg"), Some(&AudioLoop { loop_start: 12.48, loop_end: Some(96.0) }));
        let metadata = parse_audio_metadata("[\"music/other.ogg\"]\nloop_start = 4.0\n").unwrap();
        assert_eq!(metadata.get("music/other.ogg"), Some(&AudioLoop { loop_start: 4.0, loop_end: None }));
    }

    #[test]
    fn project_loads_optional_audio_metadata_from_the_assets_directory() {
        let root = std::env::temp_dir().join(format!("kairos-project-audio-{}", std::process::id()));
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::create_dir_all(root.join("assets/audio")).unwrap();
        std::fs::write(root.join("kairos.toml"), "name = \"test\"").unwrap();
        std::fs::write(root.join("script/start.kai"), "label start:\n    return\n").unwrap();
        std::fs::write(root.join("assets/audio/audio.toml"), "[\"music/theme.ogg\"]\nloop_start = 2.0\n").unwrap();
        let project = Project::load(&root).unwrap();
        assert_eq!(project.audio_metadata.get("music/theme.ogg"), Some(&AudioLoop { loop_start: 2.0, loop_end: None }));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sources_compile_in_path_order_and_report_diagnostics_by_path() {
        let sources = [
            (PathBuf::from("script/b.kai"), "label b:\n    jump nowhere\n".to_string()),
            (PathBuf::from("script/a.kai"), "label start:\n    \"hi\"\n    jump b\n".to_string()),
        ];
        let p = Project::from_sources("name = \"x\"", sources).unwrap();
        assert_eq!(p.files, [PathBuf::from("script/a.kai"), PathBuf::from("script/b.kai")]);
        assert_eq!(p.diagnostics_report(), "script/b.kai:2: unknown label `nowhere`\n");
    }

    #[test]
    fn pc_at_finds_the_first_instruction_on_or_after_a_line_in_that_file() {
        let sources = [
            (PathBuf::from("script/a.kai"), "label start:\n    \"one\"\n\n    \"two\"\n".to_string()),
            (PathBuf::from("script/b.kai"), "label b:\n    \"three\"\n".to_string()),
        ];
        let p = Project::from_sources("name = \"x\"", sources).unwrap();
        let pc = p.pc_at(Path::new("script/a.kai"), 3).unwrap();
        assert_eq!((p.program.files[pc], p.program.lines[pc]), (0, 4));
        let pc = p.pc_at(Path::new("script/b.kai"), 1).unwrap();
        assert_eq!((p.program.files[pc], p.program.lines[pc]), (1, 2));
        assert_eq!(p.pc_at(Path::new("script/a.kai"), 99), None);
        assert_eq!(p.pc_at(Path::new("script/zzz.kai"), 1), None);
    }
}

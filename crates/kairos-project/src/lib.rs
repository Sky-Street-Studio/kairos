//! Project manifests, script loading and the asset naming rule shared by the CLI, player and editor.

pub mod assets;
pub mod export;
pub mod screens;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kairos_script::Diagnostic;
use kairos_vm::Program;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerProjectJson {
    pub manifest: String,
    pub scripts: Vec<PlayerScriptJson>,
    #[serde(default)]
    pub screens: Vec<PlayerScreenJson>,
    #[serde(default)]
    pub translations: Vec<PlayerTranslationJson>,
    pub asset_base_url: String,
    #[serde(default)]
    pub assets: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerScriptJson {
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerScreenJson {
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerTranslationJson {
    pub lang: String,
    pub source: String,
}

pub type Translations = BTreeMap<String, BTreeMap<String, String>>;
pub type StaleTranslations = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default = "default_start")]
    pub start: String,
    #[serde(default)]
    pub window: Window,
    #[serde(default)]
    pub audio: AudioSettings,
    #[serde(default)]
    pub steam: Option<SteamConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SteamConfig {
    pub app_id: u32,
    #[serde(default)]
    pub achievements: BTreeMap<String, String>,
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
    pub translations: Translations,
    pub stale_translations: StaleTranslations,
    pub manifest: Manifest,
    pub program: Arc<Program>,
    /// Script paths in compile order; [`Program::files`] indexes into this list.
    pub files: Vec<PathBuf>,
    /// Diagnostics paired with the file they came from.
    pub diagnostics: Vec<(PathBuf, Diagnostic)>,
    pub audio_metadata: AudioMetadata,
    pub screens: BTreeMap<String, screens::ScreenDefinition>,
    pub screen_diagnostics: Vec<(PathBuf, screens::ScreenDiagnostic)>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("kairos.toml: {0}")]
    Manifest(#[from] toml::de::Error),
    #[error("assets/audio/audio.toml: {0}")]
    AudioMetadata(#[source] toml::de::Error),
    #[error("{0}: {1}")]
    Translations(PathBuf, toml::de::Error),
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
        (project.translations, project.stale_translations) = read_translations(root)?;
        project.root = root.to_path_buf();
        let metadata_path = root.join("assets/audio/audio.toml");
        project.audio_metadata = match std::fs::read_to_string(&metadata_path) {
            Ok(text) => parse_audio_metadata(&text).map_err(ProjectError::AudioMetadata)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AudioMetadata::new(),
            Err(e) => return Err(ProjectError::Io(metadata_path, e)),
        };
        (project.screens, project.screen_diagnostics) = read_screens(root)?;
        project.validate_screens();
        project.include_screen_diagnostics();
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
            translations: BTreeMap::new(),
            stale_translations: BTreeMap::new(),
            manifest,
            program: Arc::new(program),
            files,
            diagnostics,
            audio_metadata: AudioMetadata::new(),
            screens: BTreeMap::new(),
            screen_diagnostics: Vec::new(),
        })
    }

    /// One `path:line: message` line per diagnostic.
    pub fn validate_screens(&mut self) {
        let constants: std::collections::BTreeSet<String> =
            self.program.defines.iter().map(|(name, _)| name.clone()).collect();
        let mut known_variables: std::collections::BTreeSet<String> =
            self.program.defaults.iter().chain(&self.program.defines).map(|(name, _)| name.clone()).collect();
        for instr in &self.program.instrs {
            match instr {
                kairos_vm::Instr::Assign { name, .. } | kairos_vm::Instr::Mutate { name, .. } => {
                    known_variables.insert(name.clone());
                }
                _ => {}
            }
        }
        let labels: std::collections::BTreeSet<String> = self.program.labels.keys().cloned().collect();
        let initial_values = kairos_vm::Vm::new(self.program.clone()).ok().map(|vm| vm.variables()).unwrap_or_default();
        for (pc, instr) in self.program.instrs.iter().enumerate() {
            let id = match instr {
                kairos_vm::Instr::ShowScreen(id) | kairos_vm::Instr::HideScreen(id) => id,
                _ => continue,
            };
            if !self.screens.contains_key(id) {
                let file = self.files[self.program.files[pc]].clone();
                self.diagnostics
                    .push((file, Diagnostic::new(self.program.lines[pc], format!("unknown screen `{id}`"))));
            }
        }
        let screens: Vec<_> = self.screens.values().cloned().collect();
        for screen in screens {
            let path = PathBuf::from(format!("ui/screens/{}.toml", screen.id));
            for node in &screen.nodes {
                if let Some(binding) = &node.binding {
                    let writable = matches!(node.kind, screens::NodeType::Toggle | screens::NodeType::Slider);
                    let repeat_scope = has_repeat_parent(&screen, node);
                    validate_binding_root(
                        binding,
                        writable,
                        repeat_scope,
                        &known_variables,
                        &constants,
                        &initial_values,
                        &path,
                        &node.id,
                        &mut self.screen_diagnostics,
                    );
                    if !repeat_scope && let Some(value) = initial_binding(binding, &initial_values) {
                        let compatible = match node.kind {
                            screens::NodeType::Toggle => matches!(value, kairos_script::Value::Bool(_)),
                            screens::NodeType::Bar | screens::NodeType::Slider => {
                                matches!(value, kairos_script::Value::Int(_) | kairos_script::Value::Float(_))
                            }
                            screens::NodeType::List | screens::NodeType::Repeat => {
                                matches!(value, kairos_script::Value::List(_) | kairos_script::Value::Dict(_))
                            }
                            screens::NodeType::Image => matches!(value, kairos_script::Value::Str(_)),
                            screens::NodeType::Text => matches!(
                                value,
                                kairos_script::Value::None
                                    | kairos_script::Value::Bool(_)
                                    | kairos_script::Value::Int(_)
                                    | kairos_script::Value::Float(_)
                                    | kairos_script::Value::Str(_)
                            ),
                            _ => true,
                        };
                        if !compatible {
                            self.screen_diagnostics.push((
                                path.clone(),
                                screens::ScreenDiagnostic {
                                    node: Some(node.id.clone()),
                                    message: format!(
                                        "binding `{binding}` has an unsupported value type for this widget"
                                    ),
                                },
                            ));
                        }
                    }
                }
                if let Some(target) = &node.target {
                    validate_binding_root(
                        target,
                        true,
                        has_repeat_parent(&screen, node),
                        &known_variables,
                        &constants,
                        &initial_values,
                        &path,
                        &node.id,
                        &mut self.screen_diagnostics,
                    );
                }
                if let Some(text) = &node.text {
                    let mut rest = text.as_str();
                    while let Some((start, end)) = screens::next_interpolation(rest) {
                        validate_binding_root(
                            &rest[start + 1..end],
                            false,
                            has_repeat_parent(&screen, node),
                            &known_variables,
                            &constants,
                            &initial_values,
                            &path,
                            &node.id,
                            &mut self.screen_diagnostics,
                        );
                        rest = &rest[end + 1..];
                    }
                }
                if let Some(action) = &node.action {
                    self.validate_screen_action(
                        action,
                        &labels,
                        &known_variables,
                        &constants,
                        &initial_values,
                        &path,
                        &node.id,
                    );
                }
            }
            if let Some(action) = &screen.close_action {
                self.validate_screen_action(
                    action,
                    &labels,
                    &known_variables,
                    &constants,
                    &initial_values,
                    &path,
                    "<screen>",
                );
            }
        }
        self.screen_diagnostics
            .sort_by(|a, b| a.0.cmp(&b.0).then(a.1.node.cmp(&b.1.node)).then(a.1.message.cmp(&b.1.message)));
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_screen_action(
        &mut self,
        action: &screens::ScreenAction,
        labels: &std::collections::BTreeSet<String>,
        variables: &std::collections::BTreeSet<String>,
        constants: &std::collections::BTreeSet<String>,
        initial_values: &BTreeMap<String, kairos_script::Value>,
        path: &Path,
        node: &str,
    ) {
        match action.kind {
            screens::ActionType::Set => {
                if let Some(target) = &action.target {
                    validate_binding_root(
                        target,
                        true,
                        false,
                        variables,
                        constants,
                        initial_values,
                        path,
                        node,
                        &mut self.screen_diagnostics,
                    );
                }
                if let Some(binding) = &action.binding {
                    validate_binding_root(
                        binding,
                        false,
                        false,
                        variables,
                        constants,
                        initial_values,
                        path,
                        node,
                        &mut self.screen_diagnostics,
                    );
                }
                if let Some(value) = &action.value
                    && contains_datetime(value)
                {
                    self.screen_diagnostics.push((
                        path.to_path_buf(),
                        screens::ScreenDiagnostic {
                            node: Some(node.to_string()),
                            message: "datetime values are not supported in set actions".into(),
                        },
                    ));
                }
                if let Some(target) = &action.target
                    && let Some(target_value) = initial_binding(target, initial_values)
                {
                    let source_matches = if let Some(value) = &action.value {
                        contains_datetime(value) || toml_type_matches(value, target_value)
                    } else {
                        action
                            .binding
                            .as_deref()
                            .and_then(|binding| initial_binding(binding, initial_values))
                            .is_none_or(|value| same_value_type(value, target_value))
                    };
                    if !source_matches {
                        self.screen_diagnostics.push((
                            path.to_path_buf(),
                            screens::ScreenDiagnostic {
                                node: Some(node.to_string()),
                                message: format!("set value type is incompatible with target binding `{target}`"),
                            },
                        ));
                    }
                }
            }
            screens::ActionType::Jump | screens::ActionType::Call => {
                if let Some(label) = &action.target
                    && !labels.contains(label.as_str())
                {
                    self.screen_diagnostics.push((
                        path.to_path_buf(),
                        screens::ScreenDiagnostic {
                            node: Some(node.to_string()),
                            message: format!("unknown label `{label}`"),
                        },
                    ));
                }
            }
            screens::ActionType::Show | screens::ActionType::Hide => {
                if let Some(id) = &action.target
                    && !self.screens.contains_key(id)
                {
                    self.screen_diagnostics.push((
                        path.to_path_buf(),
                        screens::ScreenDiagnostic {
                            node: Some(node.to_string()),
                            message: format!("unknown screen `{id}`"),
                        },
                    ));
                }
            }
            screens::ActionType::Return => {}
        }
    }

    pub fn include_screen_diagnostics(&mut self) {
        for (file, diagnostic) in &self.screen_diagnostics {
            let node = diagnostic.node.as_deref().map(|id| format!("node `{id}`: ")).unwrap_or_default();
            self.diagnostics.push((file.clone(), Diagnostic::new(0, format!("{node}{}", diagnostic.message))));
        }
    }

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

    pub fn asset_index(&self) -> Result<assets::AssetIndex, ProjectError> {
        let dir = self.assets_dir();
        let paths = assets::collect_paths(&dir).map_err(|e| ProjectError::Io(dir, e))?;
        Ok(assets::AssetIndex::new(paths))
    }

    pub fn asset_diagnostics(&self) -> Result<Vec<(PathBuf, String)>, ProjectError> {
        let index = self.asset_index()?;
        let mut diagnostics = Vec::new();
        for (name, paths) in index.duplicate_images() {
            diagnostics.push((
                self.assets_dir().join(&paths[0]),
                format!("duplicate image name `{name}`: {} and {}", paths[0], paths[1]),
            ));
        }
        for (name, paths) in index.duplicate_audio() {
            diagnostics.push((
                self.assets_dir().join(&paths[0]),
                format!("duplicate audio name `{name}`: {} and {}", paths[0], paths[1]),
            ));
        }
        for (pc, instr) in self.program.instrs.iter().enumerate() {
            let kairos_vm::Instr::Show { image, .. } = instr else { continue };
            let Some(tag) = image.first() else { continue };
            let manifest = index.layer_manifest(tag);
            if !index.contains(&manifest) {
                continue;
            }
            let path = self.assets_dir().join(&manifest);
            let text = std::fs::read_to_string(&path).map_err(|e| ProjectError::Io(path.clone(), e))?;
            let config: assets::LayerConfig = match toml::from_str(&text) {
                Ok(config) => config,
                Err(error) => {
                    diagnostics.push((path, format!("invalid layered sprite config: {error}")));
                    continue;
                }
            };
            let script_path = self.files[self.program.files[pc]].clone();
            for attr in image.iter().skip(1) {
                if !config.has_attribute(tag, attr, &index) {
                    diagnostics.push((
                        script_path.clone(),
                        format!(
                            "line {}: unknown layered sprite attribute `{attr}` for `{tag}`",
                            self.program.lines[pc]
                        ),
                    ));
                }
            }
        }
        Ok(diagnostics)
    }
    pub fn set_translation(&mut self, lang: &str, source: &str) -> Result<(), ProjectError> {
        let path = PathBuf::from(format!("tl/{lang}.toml"));
        let (active, stale) = parse_translation_file(&path, source)?;
        self.translations.insert(lang.to_string(), active);
        self.stale_translations.remove(lang);
        if !stale.is_empty() {
            self.stale_translations.insert(lang.to_string(), stale);
        }
        Ok(())
    }

    pub fn translation_entries(&self) -> Vec<(String, String)> {
        let mut entries = Vec::new();
        for (pc, instr) in self.program.instrs.iter().enumerate() {
            let id = &self.program.ids[pc];
            match instr {
                kairos_vm::Instr::Say { speaker, text } => {
                    entries.push((id.clone(), text.clone()));
                    if let Some(kairos_script::Speaker::Character(character)) = speaker
                        && let Some(definition) = self.program.characters.get(character)
                    {
                        entries.push((format!("{id}#name"), definition.name.clone()));
                    }
                }
                kairos_vm::Instr::Menu { caption, choices } => {
                    if let Some((_, text)) = caption {
                        entries.push((format!("{id}#caption"), text.clone()));
                    }
                    entries.extend(
                        choices
                            .iter()
                            .enumerate()
                            .map(|(index, choice)| (format!("{id}#choice:{index}"), choice.text.clone())),
                    );
                }
                _ => {}
            }
        }
        entries
    }
}

fn read_translations(root: &Path) -> Result<(Translations, StaleTranslations), ProjectError> {
    let dir = root.join("tl");
    let paths = match std::fs::read_dir(&dir) {
        Ok(paths) => paths,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((BTreeMap::new(), BTreeMap::new()));
        }
        Err(error) => return Err(ProjectError::Io(dir, error)),
    };
    let mut translations = BTreeMap::new();
    let mut stale = BTreeMap::new();
    for entry in paths {
        let path = entry.map_err(|e| ProjectError::Io(dir.clone(), e))?.path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let lang = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        let source = std::fs::read_to_string(&path).map_err(|e| ProjectError::Io(path.clone(), e))?;
        let (active, stale_keys) = parse_translation_file(&path, &source)?;
        translations.insert(lang.clone(), active);
        if !stale_keys.is_empty() {
            stale.insert(lang, stale_keys);
        }
    }
    Ok((translations, stale))
}

fn parse_translation_file(path: &Path, source: &str) -> Result<(BTreeMap<String, String>, Vec<String>), ProjectError> {
    let parsed: toml::Value = toml::from_str(source).map_err(|e| ProjectError::Translations(path.to_path_buf(), e))?;
    let mut active = BTreeMap::new();
    let mut stale = Vec::new();
    if let Some(table) = parsed.as_table() {
        for (key, value) in table {
            if key == "stale" {
                if let Some(entries) = value.as_table() {
                    stale.extend(entries.keys().cloned());
                }
            } else if let Some(value) = value.as_str() {
                active.insert(key.clone(), value.to_string());
            }
        }
    }
    Ok((active, stale))
}

fn initial_binding<'a>(
    binding: &str,
    values: &'a BTreeMap<String, kairos_script::Value>,
) -> Option<&'a kairos_script::Value> {
    let parts = screens::parse_binding(binding).ok()?;
    let (name, rest) = screens::binding_root(&parts)?;
    screens::read_binding(values.get(&name)?, rest)
}

fn contains_datetime(value: &toml::Value) -> bool {
    match value {
        toml::Value::Datetime(_) => true,
        toml::Value::Array(values) => values.iter().any(contains_datetime),
        toml::Value::Table(values) => values.values().any(contains_datetime),
        _ => false,
    }
}

fn toml_type_matches(value: &toml::Value, target: &kairos_script::Value) -> bool {
    matches!(
        (value, target),
        (toml::Value::Boolean(_), kairos_script::Value::Bool(_))
            | (toml::Value::Integer(_), kairos_script::Value::Int(_))
            | (toml::Value::Float(_), kairos_script::Value::Float(_))
            | (toml::Value::String(_), kairos_script::Value::Str(_))
            | (toml::Value::Array(_), kairos_script::Value::List(_))
            | (toml::Value::Table(_), kairos_script::Value::Dict(_))
    )
}

fn same_value_type(left: &kairos_script::Value, right: &kairos_script::Value) -> bool {
    matches!(
        (left, right),
        (kairos_script::Value::None, kairos_script::Value::None)
            | (kairos_script::Value::Bool(_), kairos_script::Value::Bool(_))
            | (kairos_script::Value::Int(_), kairos_script::Value::Int(_))
            | (kairos_script::Value::Float(_), kairos_script::Value::Float(_))
            | (kairos_script::Value::Str(_), kairos_script::Value::Str(_))
            | (kairos_script::Value::List(_), kairos_script::Value::List(_))
            | (kairos_script::Value::Dict(_), kairos_script::Value::Dict(_))
    )
}

fn has_repeat_parent(screen: &screens::ScreenDefinition, node: &screens::ScreenNode) -> bool {
    let mut parent = node.parent.as_deref();
    while let Some(id) = parent {
        let Some(parent_node) = screen.nodes.iter().find(|candidate| candidate.id == id) else { return false };
        if parent_node.kind == screens::NodeType::Repeat {
            return true;
        }
        parent = parent_node.parent.as_deref();
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn validate_binding_root(
    binding: &str,
    writable: bool,
    repeat_scope: bool,
    variables: &std::collections::BTreeSet<String>,
    constants: &std::collections::BTreeSet<String>,
    initial_values: &BTreeMap<String, kairos_script::Value>,
    path: &Path,
    node: &str,
    diagnostics: &mut Vec<(PathBuf, screens::ScreenDiagnostic)>,
) {
    let parts = screens::parse_binding(binding).ok();
    let root = parts.as_ref().and_then(|parts| screens::binding_root(parts));
    let valid = root.is_some_and(|(root, rest)| {
        let local = matches!(root.as_str(), "item" | "index" | "key");
        if local {
            return repeat_scope && !writable;
        }
        if !variables.contains(&root) || writable && constants.contains(&root) {
            return false;
        }
        if let Some(value) = initial_values.get(&root) {
            rest.is_empty() || screens::read_binding(value, rest).is_some()
        } else {
            true
        }
    });
    if !valid {
        diagnostics.push((
            path.to_path_buf(),
            screens::ScreenDiagnostic {
                node: Some(node.to_string()),
                message: format!("binding `{binding}` does not name a declared variable"),
            },
        ));
    }
}

type ScreenDiagnostics = Vec<(PathBuf, screens::ScreenDiagnostic)>;
type ScreenDefinitions = BTreeMap<String, screens::ScreenDefinition>;

fn read_screens(root: &Path) -> Result<(ScreenDefinitions, ScreenDiagnostics), ProjectError> {
    let dir = root.join("ui/screens");
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((BTreeMap::new(), Vec::new())),
        Err(error) => return Err(ProjectError::Io(dir, error)),
    };
    let mut screens = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let mut paths: Vec<_> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "toml"))
        .collect();
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path).map_err(|error| ProjectError::Io(path.clone(), error))?;
        match screens::parse_screen(&text) {
            Ok(screen) => {
                let expected = path.file_stem().and_then(|name| name.to_str()).unwrap_or_default().to_string();
                if screen.id != expected {
                    diagnostics.push((
                        path,
                        screens::ScreenDiagnostic {
                            node: None,
                            message: format!("screen id `{}` does not match filename `{expected}`", screen.id),
                        },
                    ));
                } else if screens.insert(screen.id.clone(), screen).is_some() {
                    diagnostics
                        .push((path, screens::ScreenDiagnostic { node: None, message: "duplicate screen id".into() }));
                }
            }
            Err(errors) => diagnostics.extend(errors.into_iter().map(|error| (path.clone(), error))),
        }
    }
    Ok((screens, diagnostics))
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
    fn loading_projects_reports_screen_files_nodes_and_unknown_script_references() {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("kairos-screen-diagnostics-{nonce}"));
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::create_dir_all(root.join("ui/screens")).unwrap();
        std::fs::write(root.join("kairos.toml"), "name = \"test\"").unwrap();
        std::fs::write(
            root.join("script/main.kai"),
            r#"default selected = "key"
default persistent.inventory = ["key"]
label start:
    show screen absent
    "Ready."
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ui/screens/phone.toml"),
            r#"
            id = "phone"
            [[nodes]]
            id = "root"
            type = "column"
            [[nodes]]
            id = "items"
            parent = "root"
            type = "list"
            binding = "missing"
            target = "selected"
            [[nodes]]
            id = "display"
            parent = "root"
            type = "text"
            text = "Item: [persistent.inventory[0]]"
        "#,
        )
        .unwrap();
        let project = Project::load(&root).unwrap();
        let report = project.diagnostics_report();
        let script = Path::new("script").join("main.kai");
        assert!(report.contains(&format!("{}:4: unknown screen `absent`", script.display())), "{report}");
        assert!(report.contains("ui/screens/phone.toml:0: node `items`: binding `missing`"), "{report}");
        assert!(!report.contains("node `display`"), "{report}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn set_actions_reject_literal_values_incompatible_with_the_target_type() {
        let mut project = Project::from_sources(
            "name = \"test\"",
            [(
                PathBuf::from("script/main.kai"),
                r#"default enabled = false
default created = "today"
default label = "no"
label start:
    return
"#
                .into(),
            )],
        )
        .unwrap();
        project.screens = BTreeMap::from([(
            "phone".into(),
            screens::parse_screen(
                r#"
                id = "phone"
                [[nodes]]
                id = "root"
                type = "column"
                [[nodes]]
                id = "switch"
                parent = "root"
                type = "button"
                action = { type = "set", target = "enabled", value = "yes" }
                [[nodes]]
                id = "timestamp"
                parent = "root"
                type = "button"
                action = { type = "set", target = "created", value = 2024-01-02 }
                [[nodes]]
                id = "copy"
                parent = "root"
                type = "button"
                action = { type = "set", target = "enabled", binding = "label" }
            "#,
            )
            .unwrap(),
        )]);

        project.validate_screens();

        assert!(
            project.screen_diagnostics.iter().any(|(path, diagnostic)| {
                path == Path::new("ui/screens/phone.toml")
                    && diagnostic.node.as_deref() == Some("switch")
                    && diagnostic.message.contains("incompatible")
            }),
            "{:?}",
            project.screen_diagnostics
        );
        assert!(
            project.screen_diagnostics.iter().any(|(path, diagnostic)| {
                path == Path::new("ui/screens/phone.toml")
                    && diagnostic.node.as_deref() == Some("timestamp")
                    && diagnostic.message.contains("datetime")
            }),
            "{:?}",
            project.screen_diagnostics
        );
        assert!(
            project.screen_diagnostics.iter().any(|(path, diagnostic)| {
                path == Path::new("ui/screens/phone.toml")
                    && diagnostic.node.as_deref() == Some("copy")
                    && diagnostic.message.contains("incompatible")
            }),
            "{:?}",
            project.screen_diagnostics
        );
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
    fn translation_entries_use_statement_ids_and_choice_indexes() {
        let project = Project::from_sources(
            "name = \"x\"",
            [(
                PathBuf::from("script/a.kai"),
                r#"character e "Eileen"
label start:
    e "Hello"
    menu:
        "One":
            return
"#
                .into(),
            )],
        )
        .unwrap();
        let entries = project.translation_entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].1, "Hello");
        assert_eq!(entries[1].0, format!("{}#name", entries[0].0));
        assert_eq!(entries[2].0, format!("{}#choice:0", project.program.ids[1]));
    }

    #[test]
    fn loads_flat_translation_files_and_collects_stale_keys() {
        let root = std::env::temp_dir().join(format!("kairos-project-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::create_dir_all(root.join("tl/nested")).unwrap();
        std::fs::write(root.join("kairos.toml"), r#"name = "x""#).unwrap();
        std::fs::write(
            root.join("script/a.kai"),
            r#"label start:
    "Hello"
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("tl/ja.toml"),
            r#""some-id" = "Hola"
[stale]
"old-id" = "Adios"
"#,
        )
        .unwrap();
        std::fs::write(root.join("tl/nested/fr.toml"), r#""other" = "non-recursive""#).unwrap();
        let project = Project::load(&root).unwrap();
        assert_eq!(project.translations["ja"]["some-id"], "Hola");
        assert_eq!(project.stale_translations["ja"], ["old-id"]);
        assert!(!project.translations.contains_key("fr"));
        std::fs::remove_dir_all(root).unwrap();
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

    #[test]
    fn asset_diagnostics_report_duplicates_and_unknown_layer_attributes() {
        let root = std::env::temp_dir().join(format!("kairos-project-assets-{}", std::process::id()));
        std::fs::create_dir_all(root.join("assets/images/other")).unwrap();
        std::fs::create_dir_all(root.join("assets/images/sprites/e/face")).unwrap();
        std::fs::create_dir_all(root.join("assets/audio/music")).unwrap();
        std::fs::write(root.join("assets/images/bg room.png"), []).unwrap();
        std::fs::write(root.join("assets/images/other/bg room.webp"), []).unwrap();
        std::fs::write(root.join("assets/images/sprites/e/layers.toml"), "order = ['face']").unwrap();
        std::fs::write(root.join("assets/images/sprites/e/face/happy.png"), []).unwrap();
        std::fs::write(root.join("assets/audio/theme.ogg"), []).unwrap();
        std::fs::write(root.join("assets/audio/music/theme.mp3"), []).unwrap();
        let mut project = Project::from_sources(
            "name = 'test'",
            [(PathBuf::from("script/main.kai"), "character e \"E\"\nlabel start:\n    show e happy strange\n".into())],
        )
        .unwrap();
        project.root = root.clone();
        let messages: Vec<_> = project.asset_diagnostics().unwrap().into_iter().map(|(_, message)| message).collect();
        assert!(messages.iter().any(|message| message.contains("duplicate image name `bg room`")));
        assert!(messages.iter().any(|message| message.contains("duplicate audio name `theme`")));
        assert!(messages.iter().any(|message| message.contains("unknown layered sprite attribute `strange`")));
        std::fs::remove_dir_all(root).unwrap();
    }
}

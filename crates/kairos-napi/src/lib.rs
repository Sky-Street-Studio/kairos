//! Node bindings for the Kairos editor: diagnostics, outline, the story graph and asset usage.

mod analysis;
mod edit;
mod export;

// Unit tests link without a Node runtime, so the exported functions only exist in the addon.
#[cfg(not(test))]
mod bindings {
    use std::sync::{LazyLock, Mutex};

    use napi_derive::napi;

    use crate::analysis::{
        self, AnalyzeInput, CheckResult, Outline, ParseCache, ParseResult, ProjectAnalysis, SourceFile, StoryGraph,
    };
    use crate::export;

    static CACHE: LazyLock<Mutex<ParseCache>> = LazyLock::new(Default::default);

    #[napi]
    pub fn analyze(input: AnalyzeInput) -> ProjectAnalysis {
        let mut cache = CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        analysis::analyze(&input, &mut cache)
    }

    #[napi]
    pub fn parse(source: String) -> ParseResult {
        analysis::parse(&source)
    }

    #[napi]
    pub fn check_project(files: Vec<SourceFile>) -> CheckResult {
        analysis::check_project(&files)
    }

    #[napi]
    pub fn outline(source: String) -> Outline {
        analysis::outline(&source)
    }

    #[napi]
    pub fn story_graph(files: Vec<SourceFile>) -> StoryGraph {
        analysis::story_graph(&files)
    }

    #[napi]
    pub fn export_game(
        project: String,
        #[napi(ts_arg_type = "'web' | 'desktop'")] target: String,
        output: String,
        player_binary: Option<String>,
        player_pkg: Option<String>,
    ) -> napi::Result<String> {
        export::export_game(&project, &target, &output, player_binary.as_deref(), player_pkg.as_deref())
            .map_err(napi::Error::from_reason)
    }

    #[napi(js_name = "sceneBlocks")]
    pub fn scene_blocks(source: String, label: String) -> String {
        crate::edit::scene_blocks_json(&source, &label)
    }

    #[napi(js_name = "applyEdits")]
    pub fn apply_edits(files_json: String, ops_json: String) -> String {
        crate::edit::apply_edits_json(&files_json, &ops_json)
    }

    #[napi(js_name = "references")]
    pub fn references(files_json: String, label: String) -> String {
        crate::edit::references_json(&files_json, &label)
    }

    #[napi(js_name = "labelFromName")]
    pub fn label_from_name(name: String, taken_json: String) -> String {
        crate::edit::label_from_name_json(&name, &taken_json)
    }
}

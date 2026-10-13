//! Node bindings for the Kairos editor: diagnostics, outline and the story graph.

mod analysis;
mod export;

// Unit tests link without a Node runtime, so the exported functions only exist in the addon.
#[cfg(not(test))]
mod bindings {
    use napi_derive::napi;

    use crate::analysis::{self, CheckResult, Outline, ParseResult, SourceFile, StoryGraph};
    use crate::export;

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
}

//! Node bindings for the Kairos editor: diagnostics, outline and the story graph.

mod analysis;

// Unit tests link without a Node runtime, so the exported functions only exist in the addon.
#[cfg(not(test))]
mod bindings {
    use napi_derive::napi;

    use crate::analysis::{self, CheckResult, Outline, ParseResult, SourceFile, StoryGraph};

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
}

use kairos_script::edit::{self, Applied, Op, SourceFile};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum Outcome {
    Ok(Applied),
    Error(String),
}

pub fn scene_blocks_json(source: &str, label: &str) -> String {
    serde_json::to_string(&edit::scene_blocks(source, label)).expect("blocks serialize")
}

pub fn apply_edits_json(files: &str, ops: &str) -> String {
    let parsed = serde_json::from_str::<Vec<SourceFile>>(files)
        .map_err(|e| format!("bad files: {e}"))
        .and_then(|f| serde_json::from_str::<Vec<Op>>(ops).map(|o| (f, o)).map_err(|e| format!("bad edit: {e}")));
    let result = parsed.and_then(|(f, o)| edit::apply(&f, &o).map_err(|e| e.to_string()));
    let outcome = match result {
        Ok(applied) => Outcome::Ok(applied),
        Err(message) => Outcome::Error(message),
    };
    serde_json::to_string(&outcome).expect("outcome serializes")
}

pub fn references_json(files: &str, label: &str) -> String {
    let files: Vec<SourceFile> = serde_json::from_str(files).unwrap_or_default();
    serde_json::to_string(&edit::references(&files, label)).expect("references serialize")
}

pub fn label_from_name_json(name: &str, taken: &str) -> String {
    let taken: Vec<String> = serde_json::from_str(taken).unwrap_or_default();
    edit::label_from_name(name, &taken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shapes_match_the_editor_types() {
        let blocks = scene_blocks_json("label start:\n    e \"Hi\"\n", "start");
        assert_eq!(
            blocks,
            r#"{"label":"start","line":1,"blocks":[{"line":2,"endLine":2,"kind":"dialogue","character":"e","text":"Hi"}]}"#
        );
        let files = r#"[{"path":"a.kai","source":"label start:\n    \"A\"\n"}]"#;
        let ops = r#"[{"op":"insert","path":"a.kai","place":{"at":"after","line":2},"block":{"kind":"end"}}]"#;
        assert_eq!(
            apply_edits_json(files, ops),
            r#"{"ok":{"files":[{"path":"a.kai","source":"label start:\n    \"A\"\n    return\n"}],"focus":{"path":"a.kai","line":3}}}"#
        );
        assert!(apply_edits_json(files, r#"[{"op":"delete","path":"a.kai","line":9}]"#).starts_with(r#"{"error":"#));
        let jump = r#"[{"path":"a.kai","source":"label start:\n    jump end\nlabel end:\n    return\n"}]"#;
        assert_eq!(references_json(jump, "end"), r#"[{"path":"a.kai","line":2,"kind":"jump","choice":null}]"#);
        assert_eq!(label_from_name_json("Beach Day", "[]"), "beach_day");
    }
}

use std::path::Path;

use kairos_cli::{Project, run_headless};

#[test]
fn hello_kairos_matches_its_golden_transcript() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello-kairos");
    let project = Project::load(&root).unwrap();
    assert!(project.diagnostics.is_empty(), "{:?}", project.diagnostics);
    let inputs = std::fs::read_to_string(root.join("inputs.txt")).unwrap();
    let expected = std::fs::read_to_string(root.join("expected.txt")).unwrap();
    assert_eq!(run_headless(&project, &inputs).unwrap(), expected);
}

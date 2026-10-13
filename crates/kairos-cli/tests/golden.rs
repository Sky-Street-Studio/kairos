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

#[test]
fn beginner_templates_load_without_diagnostics() {
    let templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/beginner-templates");
    for entry in std::fs::read_dir(&templates).unwrap() {
        let root = entry.unwrap().path();
        let project = Project::load(&root).unwrap();
        assert!(project.diagnostics.is_empty(), "{}: {:?}", root.display(), project.diagnostics);
        assert!(project.asset_diagnostics().unwrap().is_empty(), "{}", root.display());
    }
}

#[test]
fn test_routes_emits_json_and_returns_failure_for_unreachable_labels() {
    let root = std::env::temp_dir().join(format!("kairos-routes-{}", std::process::id()));
    std::fs::create_dir_all(root.join("script")).unwrap();
    std::fs::write(root.join("kairos.toml"), "name = 'test'").unwrap();
    std::fs::write(root.join("script/main.kai"), "label start:\n    return\nlabel orphan:\n    return\n").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_kairos"))
        .args(["test-routes", root.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["unreachableLabels"][0], "orphan");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn check_reports_duplicate_asset_names_with_both_paths() {
    let root = std::env::temp_dir().join(format!("kairos-check-assets-{}", std::process::id()));
    std::fs::create_dir_all(root.join("script")).unwrap();
    std::fs::create_dir_all(root.join("assets/images/backgrounds")).unwrap();
    std::fs::create_dir_all(root.join("assets/audio/music")).unwrap();
    std::fs::write(root.join("kairos.toml"), "name = 'test'").unwrap();
    std::fs::write(root.join("script/main.kai"), "label start:\n    return\n").unwrap();
    std::fs::write(root.join("assets/images/bg room.png"), []).unwrap();
    std::fs::write(root.join("assets/images/backgrounds/bg room.webp"), []).unwrap();
    std::fs::write(root.join("assets/audio/theme.ogg"), []).unwrap();
    std::fs::write(root.join("assets/audio/music/theme.mp3"), []).unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_kairos")).arg("check").arg(&root).output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!output.status.success());
    assert!(stderr.contains("images/bg room.png"), "{stderr}");
    assert!(stderr.contains("images/backgrounds/bg room.webp"), "{stderr}");
    assert!(stderr.contains("audio/theme.ogg"), "{stderr}");
    assert!(stderr.contains("audio/music/theme.mp3"), "{stderr}");
    std::fs::remove_dir_all(root).unwrap();
}

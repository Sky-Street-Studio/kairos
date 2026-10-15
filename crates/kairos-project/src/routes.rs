use std::collections::{BTreeMap, BTreeSet, VecDeque};

use kairos_vm::{Event, Instr, RouteStep, Vm};
use serde::{Deserialize, Serialize};

use crate::{
    Project,
    assets::{AssetIndex, LayerConfig, collect_paths},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RouteTestOptions {
    pub max_depth: usize,
    pub max_loop_revisits: usize,
    pub max_routes: usize,
}

impl Default for RouteTestOptions {
    fn default() -> Self {
        Self { max_depth: 1000, max_loop_revisits: 3, max_routes: 10000 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteIssue {
    pub label: String,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteTestReport {
    pub reachable_labels: Vec<String>,
    pub unreachable_labels: Vec<String>,
    pub dead_ends: Vec<RouteIssue>,
    pub missing_assets: Vec<RouteIssue>,
    pub unresolved_choices: Vec<RouteIssue>,
    pub truncated: Vec<String>,
    pub routes_explored: usize,
}

impl RouteTestReport {
    pub fn has_problems(&self) -> bool {
        !self.unreachable_labels.is_empty()
            || !self.dead_ends.is_empty()
            || !self.missing_assets.is_empty()
            || !self.unresolved_choices.is_empty()
            || !self.truncated.is_empty()
    }
}

struct Work {
    vm: Vm,
    depth: usize,
    visits: BTreeMap<String, usize>,
    last_label: Option<String>,
    route: Vec<String>,
}

pub fn test_routes(
    project: &Project,
    options: RouteTestOptions,
) -> Result<RouteTestReport, Box<dyn std::error::Error>> {
    let assets = AssetIndex::new(collect_paths(&project.root.join("assets"))?);
    let mut work = VecDeque::new();
    let mut entries = BTreeSet::from([project.manifest.start.as_str()]);
    for entry in ["splashscreen", "main_menu"] {
        if project.program.labels.contains_key(entry) {
            entries.insert(entry);
        }
    }
    let mut routes_scheduled = 0;
    let mut truncated = BTreeSet::new();
    for entry in entries {
        if routes_scheduled >= options.max_routes {
            truncated.insert(format!("route limit ({}) reached", options.max_routes));
            break;
        }
        let mut vm = Vm::new(project.program.clone())?;
        vm.start(entry)?;
        work.push_back(Work { vm, depth: 0, visits: BTreeMap::new(), last_label: None, route: Vec::new() });
        routes_scheduled += 1;
    }
    let mut reachable = BTreeSet::new();
    let mut dead_ends = BTreeMap::new();
    let mut missing_assets = BTreeMap::new();
    let mut unresolved_choices = BTreeMap::new();
    let mut routes_explored = 0;

    while let Some(mut item) = work.pop_front() {
        routes_explored += 1;
        loop {
            if item.depth >= options.max_depth {
                truncated.insert(format!("depth limit ({}) reached at {}", options.max_depth, current_label(&item.vm)));
                if let Some(choice) = item.route.last() {
                    unresolved_choices.insert(
                        (choice.clone(), item.vm.current_line()),
                        RouteIssue {
                            label: current_label(&item.vm),
                            line: item.vm.current_line(),
                            message: "route did not resolve before the depth limit".into(),
                        },
                    );
                }
                break;
            }
            let label = current_label(&item.vm);
            reachable.insert(label.clone());
            let step = item.vm.advance_route();
            item.vm.clear_history();
            let line = item.vm.program().lines.get(item.vm.last_pc()).copied().unwrap_or_default();
            let trace = item.vm.take_route_labels();
            reachable.extend(trace.iter().cloned());
            for pc in item.vm.take_route_asset_pcs() {
                let (missing, resource) = match &item.vm.program().instrs[pc] {
                    Instr::Scene { image: Some(words), .. } => {
                        (assets.image_candidates(words).is_empty(), words.join(" "))
                    }
                    Instr::Show { image, .. } => (image_paths(project, &assets, image).is_empty(), image.join(" ")),
                    Instr::Play { asset, .. } => (assets.audio_candidates(asset).is_empty(), asset.clone()),
                    _ => continue,
                };
                if missing {
                    let asset_label = item.vm.program().label_at(pc).unwrap_or("<unknown>").to_string();
                    let asset_line = item.vm.program().lines[pc];
                    missing_assets.insert(
                        (asset_label.clone(), asset_line, resource.clone()),
                        issue(asset_label, asset_line, format!("missing asset `{resource}`")),
                    );
                }
            }
            let mut revisit_limit = false;
            for seen_label in trace {
                if item.last_label.as_deref() != Some(&seen_label) {
                    let visits = item.visits.entry(seen_label.clone()).or_default();
                    *visits += 1;
                    item.last_label = Some(seen_label.clone());
                    if *visits > options.max_loop_revisits.max(1) {
                        truncated.insert(format!(
                            "loop revisit limit ({}) reached at {seen_label}",
                            options.max_loop_revisits
                        ));
                        revisit_limit = true;
                        break;
                    }
                }
            }
            let label = item.last_label.clone().unwrap_or(label);
            if revisit_limit {
                if let Some(choice) = item.route.last() {
                    unresolved_choices.insert(
                        (choice.clone(), line),
                        issue(label, line, "route did not resolve before the loop revisit limit"),
                    );
                }
                break;
            }
            match step {
                Ok(RouteStep::Conditional { pc, .. }) => {
                    let mut other = Work {
                        vm: item.vm.clone(),
                        depth: item.depth + 1,
                        visits: item.visits.clone(),
                        last_label: item.last_label.clone(),
                        route: item.route.clone(),
                    };
                    if item.vm.take_route_branch(pc, true).is_ok() && other.vm.take_route_branch(pc, false).is_ok() {
                        item.depth += 1;
                        if routes_scheduled < options.max_routes {
                            work.push_back(other);
                            routes_scheduled += 1;
                        } else {
                            truncated.insert(format!("route limit ({}) reached", options.max_routes));
                        }
                        continue;
                    }
                    dead_ends
                        .insert((label.clone(), line), issue(label, line, "conditional branch could not be explored"));
                    break;
                }
                Ok(RouteStep::Event(event)) => match event {
                    Event::Menu { choices, .. } => {
                        for (index, choice) in choices.into_iter().enumerate() {
                            if routes_scheduled >= options.max_routes {
                                truncated.insert(format!("route limit ({}) reached", options.max_routes));
                                break;
                            }
                            let mut branch = item.vm.clone();
                            if let Err(error) = branch.choose(index) {
                                unresolved_choices.insert(
                                    (choice.clone(), line),
                                    issue(
                                        label.clone(),
                                        line,
                                        format!("choice `{choice}` could not be resolved: {error}"),
                                    ),
                                );
                                continue;
                            }
                            let mut route = item.route.clone();
                            route.push(choice);
                            work.push_back(Work {
                                vm: branch,
                                depth: item.depth + 1,
                                visits: item.visits.clone(),
                                last_label: item.last_label.clone(),
                                route,
                            });
                            routes_scheduled += 1;
                        }
                        break;
                    }
                    Event::End => {
                        if item.vm.program().lines.get(item.vm.last_pc()) == Some(&0) {
                            let message = "route falls off the script without an explicit return or quit";
                            dead_ends.insert((label.clone(), line), issue(label.clone(), line, message));
                            if let Some(choice) = item.route.last() {
                                unresolved_choices.insert((choice.clone(), line), issue(label, line, message));
                            }
                        }
                        break;
                    }
                    Event::Quit => break,
                    _ => item.depth += 1,
                },
                Err(error) => {
                    let message = error.to_string();
                    if message.contains("script ran too long without an interaction") {
                        truncated.insert(format!("VM instruction limit reached at {label}:{line}"));
                    } else {
                        dead_ends.insert((label.clone(), line), issue(label.clone(), line, message.clone()));
                    }
                    if let Some(choice) = item.route.last() {
                        unresolved_choices.insert(
                            (choice.clone(), line),
                            issue(label, line, format!("choice `{choice}` ends in a runtime error: {message}")),
                        );
                    }
                    break;
                }
            }
        }
    }

    let all_labels: BTreeSet<_> = project.program.labels.keys().cloned().collect();
    Ok(RouteTestReport {
        reachable_labels: reachable.iter().cloned().collect(),
        unreachable_labels: all_labels.difference(&reachable).cloned().collect(),
        dead_ends: dead_ends.into_values().collect(),
        missing_assets: missing_assets.into_values().collect(),
        unresolved_choices: unresolved_choices.into_values().collect(),
        truncated: truncated.into_iter().collect(),
        routes_explored,
    })
}

fn image_paths(project: &Project, index: &AssetIndex, image: &[String]) -> Vec<String> {
    let Some(tag) = image.first() else { return Vec::new() };
    let manifest = index.layer_manifest(tag);
    if index.contains(&manifest) {
        let path = project.assets_dir().join(&manifest);
        let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
        let Ok(config) = toml::from_str::<LayerConfig>(&text) else { return Vec::new() };
        config.files(tag, &image[1..], index)
    } else {
        index.image_candidates(image)
    }
}

fn current_label(vm: &Vm) -> String {
    vm.program().label_at(vm.state().pc).unwrap_or("<unknown>").to_string()
}

fn issue(label: String, line: usize, message: impl Into<String>) -> RouteIssue {
    RouteIssue { label, line, message: message.into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn project(source: &str) -> Project {
        Project::from_sources("name = \"test\"", [(PathBuf::from("script/main.kai"), source.to_string())]).unwrap()
    }

    #[test]
    fn explores_both_conditional_paths_and_reports_missing_assets_and_unreachable_labels() {
        let project = project(
            "default flag = False\nlabel start:\n    if flag:\n        jump yes\n    else:\n        jump no\nlabel yes:\n    scene missing background\n    return\nlabel no:\n    return\nlabel orphan:\n    return\n",
        );
        let report = test_routes(&project, RouteTestOptions::default()).unwrap();
        assert_eq!(report.reachable_labels, ["no", "start", "yes"]);
        assert_eq!(report.unreachable_labels, ["orphan"]);
        assert_eq!(report.missing_assets.len(), 1);
        assert!(report.has_problems());
    }

    #[test]
    fn reports_a_script_that_falls_off_without_an_explicit_terminal() {
        let project = project("label start:\n    \"unfinished\"\n");
        let report = test_routes(&project, RouteTestOptions::default()).unwrap();
        assert_eq!(report.dead_ends.len(), 1);
        assert!(report.dead_ends[0].message.contains("without an explicit return or quit"));
    }

    #[test]
    fn layered_show_resolves_assets_from_its_layer_config() {
        let root = std::env::temp_dir().join(format!("kairos-layered-route-{}", std::process::id()));
        std::fs::create_dir_all(root.join("script")).unwrap();
        std::fs::create_dir_all(root.join("assets/images/sprites/hero/face")).unwrap();
        std::fs::write(root.join("kairos.toml"), "name = 'test'").unwrap();
        std::fs::write(root.join("script/main.kai"), "label start:\n    show hero happy\n    return\n").unwrap();
        std::fs::write(root.join("assets/images/sprites/hero/layers.toml"), "order = ['face']\n").unwrap();
        std::fs::write(root.join("assets/images/sprites/hero/face/happy.png"), []).unwrap();

        let project = Project::load(&root).unwrap();
        let report = test_routes(&project, RouteTestOptions::default()).unwrap();

        assert!(report.missing_assets.is_empty(), "{:?}", report.missing_assets);
        assert!(!report.has_problems());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn max_routes_caps_menu_enqueues_and_reports_truncation() {
        let menu = project(
            "label start:\n    \"First\"\n    menu:\n        \"first\":\n            return\n        \"second\":\n            return\n        \"third\":\n            return\n",
        );
        let report = test_routes(&menu, RouteTestOptions { max_routes: 2, ..Default::default() }).unwrap();

        assert_eq!(report.routes_explored, 2);
        assert_eq!(report.truncated, ["route limit (2) reached"]);
    }

    #[test]
    fn explores_every_menu_choice_and_truncates_loops() {
        let menu = project(
            "label start:\n    menu:\n        \"first\":\n            jump ending\n        \"second\":\n            return\nlabel ending:\n    return\n",
        );
        let report = test_routes(&menu, RouteTestOptions::default()).unwrap();
        assert_eq!(report.routes_explored, 3);
        assert!(report.unresolved_choices.is_empty());

        let looping = project("label start:\n    jump start\n");
        let report = test_routes(&looping, RouteTestOptions { max_loop_revisits: 1, ..Default::default() }).unwrap();
        assert!(!report.truncated.is_empty());
    }
}

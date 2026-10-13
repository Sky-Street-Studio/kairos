use std::collections::{BTreeMap, BTreeSet};

use kairos_script::Value;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenDefinition {
    pub id: String,
    #[serde(default)]
    pub modal: bool,
    #[serde(default)]
    pub layer: i32,
    #[serde(default)]
    pub close_action: Option<ScreenAction>,
    pub nodes: Vec<ScreenNode>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenNode {
    pub id: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(rename = "type")]
    pub kind: NodeType,
    #[serde(default)]
    pub gap: f32,
    #[serde(default)]
    pub padding: f32,
    #[serde(default)]
    pub columns: Option<usize>,
    #[serde(default)]
    pub width: Option<Dimension>,
    #[serde(default)]
    pub height: Option<Dimension>,
    #[serde(default)]
    pub anchor_x: Option<AnchorX>,
    #[serde(default)]
    pub anchor_y: Option<AnchorY>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub binding: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub action: Option<ScreenAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum Dimension {
    Pixels(f32),
    Mode(SizingMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SizingMode {
    Fill,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorX {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorY {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Stack,
    Row,
    Column,
    Grid,
    Text,
    Image,
    Button,
    Bar,
    Toggle,
    Slider,
    List,
    Repeat,
}

#[derive(Debug, Clone, PartialEq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenAction {
    #[serde(rename = "type")]
    pub kind: ActionType,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub value: Option<toml::Value>,
    #[serde(default)]
    pub binding: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionType {
    Set,
    Jump,
    Call,
    Show,
    Hide,
    Return,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenDiagnostic {
    pub node: Option<String>,
    pub message: String,
}

impl ScreenDiagnostic {
    fn screen(message: impl Into<String>) -> Self {
        Self { node: None, message: message.into() }
    }

    fn node(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self { node: Some(id.into()), message: message.into() }
    }
}

pub fn parse_screen(source: &str) -> Result<ScreenDefinition, Vec<ScreenDiagnostic>> {
    let screen: ScreenDefinition = toml::from_str(source).map_err(|error| {
        let message = error.message().to_string();
        let node = error_node(source, &message, error.span().map(|span| span.start));

        vec![ScreenDiagnostic { node, message }]
    })?;
    validate_screen(&screen)?;
    Ok(screen)
}

pub fn validate_screen(screen: &ScreenDefinition) -> Result<(), Vec<ScreenDiagnostic>> {
    let mut errors = Vec::new();
    if !valid_id(&screen.id) {
        errors.push(ScreenDiagnostic::screen(format!("invalid screen id `{}`", screen.id)));
    }
    let mut ids = BTreeSet::new();
    for node in &screen.nodes {
        if !valid_id(&node.id) {
            errors.push(ScreenDiagnostic::node(&node.id, "invalid node id"));
        }
        if !ids.insert(node.id.as_str()) {
            errors.push(ScreenDiagnostic::node(&node.id, "duplicate node id"));
        }
        if !node.gap.is_finite() || node.gap < 0.0 || !node.padding.is_finite() || node.padding < 0.0 {
            errors.push(ScreenDiagnostic::node(&node.id, "gap and padding must be finite and non-negative"));
        }
        if node.width.is_some_and(|size| matches!(size, Dimension::Pixels(value) if !value.is_finite() || value <= 0.0))
            || node
                .height
                .is_some_and(|size| matches!(size, Dimension::Pixels(value) if !value.is_finite() || value <= 0.0))
        {
            errors.push(ScreenDiagnostic::node(&node.id, "width and height must be finite and positive"));
        }
        if node.min.zip(node.max).is_some_and(|(min, max)| !min.is_finite() || !max.is_finite() || min >= max) {
            errors.push(ScreenDiagnostic::node(&node.id, "min must be finite and less than max"));
        }
        if node.color.as_deref().is_some_and(|color| !valid_color(color)) {
            errors.push(ScreenDiagnostic::node(&node.id, "color must be #rgb, #rrggbb or #rrggbbaa"));
        }
        if node.kind == NodeType::Grid && node.columns.unwrap_or(0) == 0 {
            errors.push(ScreenDiagnostic::node(&node.id, "grid requires a positive columns value"));
        }
        if node.kind != NodeType::Grid && node.columns.is_some() {
            errors.push(ScreenDiagnostic::node(&node.id, "columns is only valid on grid nodes"));
        }
        if matches!(node.kind, NodeType::Bar | NodeType::Slider) && (node.min.is_none() || node.max.is_none()) {
            errors.push(ScreenDiagnostic::node(&node.id, "bar and slider require min and max"));
        }
        if matches!(node.kind, NodeType::Toggle | NodeType::Bar | NodeType::Slider | NodeType::List | NodeType::Repeat)
            && node.binding.is_none()
        {
            errors.push(ScreenDiagnostic::node(&node.id, "widget requires a binding"));
        }
        if let Some(binding) = &node.binding
            && parse_binding(binding).is_err()
        {
            errors.push(ScreenDiagnostic::node(&node.id, format!("invalid binding `{binding}`")));
        }
        if let Some(target) = &node.target
            && parse_binding(target).is_err()
        {
            errors.push(ScreenDiagnostic::node(&node.id, format!("invalid target binding `{target}`")));
        }
        let container =
            matches!(node.kind, NodeType::Stack | NodeType::Row | NodeType::Column | NodeType::Grid | NodeType::Repeat);
        if (node.gap != 0.0 || node.padding != 0.0) && !container {
            errors.push(ScreenDiagnostic::node(&node.id, "gap and padding are only valid on containers"));
        }
        if node.text.is_some() && !matches!(node.kind, NodeType::Text | NodeType::Button | NodeType::Toggle) {
            errors.push(ScreenDiagnostic::node(&node.id, "text is not supported by this widget"));
        }
        if node.image.is_some() && node.kind != NodeType::Image {
            errors.push(ScreenDiagnostic::node(&node.id, "image is only valid on image widgets"));
        }
        if node.binding.is_some()
            && !matches!(
                node.kind,
                NodeType::Text
                    | NodeType::Image
                    | NodeType::Bar
                    | NodeType::Toggle
                    | NodeType::Slider
                    | NodeType::List
                    | NodeType::Repeat
            )
        {
            errors.push(ScreenDiagnostic::node(&node.id, "binding is not supported by this widget"));
        }
        if node.target.is_some() && node.kind != NodeType::List {
            errors.push(ScreenDiagnostic::node(&node.id, "target is only valid on list widgets"));
        }
        if (node.min.is_some() || node.max.is_some()) && !matches!(node.kind, NodeType::Bar | NodeType::Slider) {
            errors.push(ScreenDiagnostic::node(&node.id, "min and max are only valid on bars and sliders"));
        }
        if node.action.is_some()
            && !matches!(node.kind, NodeType::Button | NodeType::Toggle | NodeType::Slider | NodeType::List)
        {
            errors.push(ScreenDiagnostic::node(&node.id, "action is only valid on interactive widgets"));
        }
        if node.kind == NodeType::List && node.target.is_none() {
            errors.push(ScreenDiagnostic::node(&node.id, "list requires a target binding"));
        }
        if let Some(action) = &node.action {
            validate_action(action, &node.id, &mut errors);
        }
    }
    for node in &screen.nodes {
        if let Some(parent) = &node.parent {
            if !ids.contains(parent.as_str()) {
                errors.push(ScreenDiagnostic::node(&node.id, format!("unknown parent `{parent}`")));
            } else if parent == &node.id {
                errors.push(ScreenDiagnostic::node(&node.id, "node cannot parent itself"));
            } else if screen.nodes.iter().find(|n| n.id == *parent).is_some_and(|p| {
                !matches!(
                    p.kind,
                    NodeType::Stack | NodeType::Row | NodeType::Column | NodeType::Grid | NodeType::Repeat
                )
            }) {
                errors.push(ScreenDiagnostic::node(&node.id, format!("parent `{parent}` is not a container")));
            }
        }
    }
    if screen.nodes.iter().filter(|node| node.parent.is_none()).count() != 1 {
        errors.push(ScreenDiagnostic::screen("screen must have exactly one root node"));
    }
    let parents: BTreeMap<_, _> = screen.nodes.iter().map(|node| (node.id.as_str(), node.parent.as_deref())).collect();
    for node in &screen.nodes {
        let mut seen = BTreeSet::new();
        let mut parent = parents.get(node.id.as_str()).copied().flatten();
        while let Some(id) = parent {
            if !seen.insert(id) || id == node.id {
                errors.push(ScreenDiagnostic::node(&node.id, "cyclic parent chain"));
                break;
            }
            parent = parents.get(id).copied().flatten();
        }
    }
    if let Some(action) = &screen.close_action {
        validate_action(action, "<screen>", &mut errors);
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn error_node(source: &str, message: &str, offset: Option<usize>) -> Option<String> {
    let field = message.split("unknown field `").nth(1).and_then(|tail| tail.split('`').next());
    let mut line_offset = 0;
    for line in source.lines() {
        if field
            .is_some_and(|field| line.trim().strip_prefix(field).is_some_and(|rest| rest.trim_start().starts_with('=')))
        {
            return node_at(source, line_offset);
        }
        line_offset += line.len() + 1;
    }
    offset.and_then(|offset| node_at(source, offset))
}

fn node_at(source: &str, offset: usize) -> Option<String> {
    let mut in_nodes = false;
    let mut id = None;
    for line in source[..offset.min(source.len())].lines() {
        let line = line.trim();
        if line == "[[nodes]]" {
            in_nodes = true;
            id = None;
        } else if in_nodes
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "id"
        {
            id = Some(value.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    id
}

fn validate_action(action: &ScreenAction, node: &str, errors: &mut Vec<ScreenDiagnostic>) {
    let expected = match action.kind {
        ActionType::Set => action.target.is_some() && (action.value.is_some() ^ action.binding.is_some()),
        ActionType::Jump | ActionType::Call | ActionType::Show | ActionType::Hide => {
            action.target.is_some() && action.value.is_none() && action.binding.is_none()
        }
        ActionType::Return => action.target.is_none() && action.value.is_none() && action.binding.is_none(),
    };
    if !expected {
        errors.push(ScreenDiagnostic::node(node, "action has missing or incompatible fields"));
    }
    for binding in action.target.iter().chain(action.binding.iter()) {
        if matches!(action.kind, ActionType::Set) && parse_binding(binding).is_err() {
            errors.push(ScreenDiagnostic::node(node, format!("invalid action binding `{binding}`")));
        }
    }
}

fn valid_color(color: &str) -> bool {
    let Some(hex) = color.strip_prefix('#') else { return false };
    matches!(hex.len(), 3 | 6 | 8) && hex.chars().all(|character| character.is_ascii_hexdigit())
}

fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingPart {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingParseError;

pub fn next_interpolation(text: &str) -> Option<(usize, usize)> {
    let start = text.find('[')?;
    let mut depth = 0;
    let mut quoted = false;
    for (offset, character) in text[start + 1..].char_indices() {
        match character {
            '"' => quoted = !quoted,
            '[' if !quoted => depth += 1,
            ']' if !quoted && depth == 0 => return Some((start, start + 1 + offset)),
            ']' if !quoted => depth -= 1,
            _ => {}
        }
    }
    None
}

pub fn parse_binding(path: &str) -> Result<Vec<BindingPart>, BindingParseError> {
    let mut parts = Vec::new();
    let mut chars = path.chars().peekable();
    let mut key = String::new();
    let mut after_index = false;
    let mut needs_segment = false;
    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                if !key.is_empty() {
                    parts.push(BindingPart::Key(std::mem::take(&mut key)));
                } else if !after_index {
                    return Err(BindingParseError);
                }
                after_index = false;
                needs_segment = true;
            }
            '[' => {
                if !key.is_empty() {
                    parts.push(BindingPart::Key(std::mem::take(&mut key)));
                }
                let mut token = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == ']' {
                        closed = true;
                        break;
                    }
                    token.push(next);
                }
                if token.is_empty() || !closed {
                    return Err(BindingParseError);
                }
                if let Ok(index) = token.parse::<usize>() {
                    parts.push(BindingPart::Index(index));
                } else if token.starts_with('"') && token.ends_with('"') && token.len() >= 2 {
                    parts.push(BindingPart::Key(token[1..token.len() - 1].to_string()));
                } else {
                    return Err(BindingParseError);
                }
                after_index = true;
                needs_segment = false;
            }
            c if c.is_ascii_alphanumeric() || c == '_' => {
                if after_index {
                    return Err(BindingParseError);
                }
                needs_segment = false;
                key.push(c);
            }
            _ => return Err(BindingParseError),
        }
    }
    if !key.is_empty() {
        parts.push(BindingPart::Key(key));
    }
    if parts.is_empty() || needs_segment { Err(BindingParseError) } else { Ok(parts) }
}

pub fn binding_root(parts: &[BindingPart]) -> Option<(String, &[BindingPart])> {
    let [BindingPart::Key(root), rest @ ..] = parts else { return None };
    if root == "persistent" {
        let [BindingPart::Key(name), rest @ ..] = rest else { return None };
        Some((format!("persistent.{name}"), rest))
    } else {
        Some((root.clone(), rest))
    }
}

pub fn read_binding<'a>(value: &'a Value, path: &[BindingPart]) -> Option<&'a Value> {
    let mut value = value;
    for part in path {
        value = match (part, value) {
            (BindingPart::Key(key), Value::Dict(dict)) => dict.get(key)?,
            (BindingPart::Index(index), Value::List(list)) => list.get(*index)?,
            (BindingPart::Key(_), _) | (BindingPart::Index(_), _) => return None,
        };
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates_a_screen_tree() {
        let screen = parse_screen(
            r#"
            id = "phone"
            [[nodes]]
            id = "root"
            type = "column"
            [[nodes]]
            id = "label"
            parent = "root"
            type = "text"
            text = "Inventory"
        "#,
        )
        .unwrap();
        assert_eq!(screen.id, "phone");
    }

    #[test]
    fn reports_invalid_nodes_and_unknown_keys() {
        let unknown = parse_screen("id='x'\n[[nodes]]\nid='root'\ntype='column'\nextra=true\n").unwrap_err();
        assert_eq!(unknown[0].node.as_deref(), Some("root"), "{unknown:?}");
        assert!(unknown[0].message.contains("unknown field `extra`"));
        let bad = parse_screen(
            "id='x'\n[[nodes]]\nid='root'\ntype='column'\n[[nodes]]\nid='x'\nparent='absent'\ntype='text'\n",
        )
        .unwrap_err();
        assert!(bad.iter().any(|error| error.node.as_deref() == Some("x") && error.message.contains("unknown parent")));
    }

    #[test]
    fn parses_nested_binding_paths_without_coercion() {
        let path = parse_binding("inventory[0].name").unwrap();
        let value = Value::Dict(BTreeMap::from([(
            "inventory".into(),
            Value::List(vec![Value::Dict(BTreeMap::from([("name".into(), Value::Str("key".into()))]))]),
        )]));
        assert_eq!(read_binding(&value, &path), Some(&Value::Str("key".into())));
        let persistent = parse_binding("persistent.inventory[0]").unwrap();
        let (root, rest) = binding_root(&persistent).unwrap();
        assert_eq!(root, "persistent.inventory");
        assert_eq!(read_binding(&Value::List(vec![Value::Str("key".into())]), rest), Some(&Value::Str("key".into())));
        assert!(parse_binding("inventory[nope]").is_err());
        let text = "Item: [inventory[0].name]!";
        let (start, end) = next_interpolation(text).unwrap();
        assert_eq!(&text[start + 1..end], "inventory[0].name");
    }
}

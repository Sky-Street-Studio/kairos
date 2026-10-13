# Custom screens contract

Custom screens add project-authored interfaces to the player without putting layout code in Kairos scripts. A screen can show game data, collect input and move the story forward. The script remains the source of game state; the screen file describes presentation and interaction.

## Goals

- Let authors build menus, phones, inventories, maps, stat pages, settings and small interactive sequences.
- Bind controls to script variables so displayed values and player input use the same state as story logic.
- Keep screen definitions portable with the project and usable by the native and web players.
- Make layout, navigation, save behavior and validation predictable before the editor or runtime implementation begins.

## Non-goals

- A general-purpose animation, drawing or game UI framework.
- Arbitrary script expressions embedded in TOML.
- A replacement for story labels, dialogue choices or the existing title and theme files.
- A visual editor in the first runtime implementation. The editor is a later authoring surface for the same files.

## Files

Each screen gets one TOML file at `ui/screens/<id>.toml`. The filename, screen `id`, and script reference use the same lowercase identifier. The directory sits beside `ui/theme.toml` and `ui/title.toml`. Missing screen files are valid; a referenced but missing screen is a project diagnostic.

A file contains screen metadata and an ordered tree of layout and widget nodes. Node IDs are unique within that screen. A node has a type, optional parent, layout properties, and type-specific properties. Child order in the file is the default visual and focus order. Unknown keys, duplicate IDs, invalid parents, unsupported widget properties and invalid bindings fail project validation with the file and key in the diagnostic.

```toml
id = "phone"
modal = true
layer = 10

[[nodes]]
id = "root"
type = "column"
gap = 12
padding = 24

[[nodes]]
id = "heading"
parent = "root"
type = "text"
text = "Messages"

[[nodes]]
id = "close"
parent = "root"
type = "button"
text = "Close"

[nodes.action]
type = "return"
```

The exact schema should stay small: metadata, a node tree, layout properties, widget properties and an optional action per interactive widget. Screens do not load scripts or define new variables.

## Layout

Every screen has one root layout. Containers arrange child nodes; widgets occupy space inside their parent.

- `stack` places children in document order on top of one another. Later children draw above earlier children.
- `row` places children horizontally.
- `column` places children vertically.
- `grid` places children in rows and columns with an explicit column count and optional gaps.

Containers support `gap` and `padding`. Row, column and grid children can use `fixed`, `fill` or `content` sizing. `fixed` uses the declared logical-pixel size, `fill` shares remaining parent space, and `content` uses the widget's measured size. Grid children fill their cells unless they specify a fixed size.

A node can anchor to the top, center or bottom vertically and left, center or right horizontally. Anchors position a node within its parent; they do not change its size. The root uses the player's logical viewport, with aspect-ratio handling owned by the player. Layout values use logical pixels and scale with the existing viewport rules. Layout overflow clips at the screen viewport. The first implementation does not support arbitrary coordinates or responsive breakpoints.

## Widgets

- `text` displays literal text or a binding. Text interpolation can reference scalar values by variable name. `None` renders as empty; lists and dictionaries require a repeater.
- `image` displays an image asset resolved by the existing asset lookup rules. An optional string binding supplies the image name.
- `button` displays a label and runs one action on activation. Disabled state can be bound to a boolean.
- `bar` displays an integer or float within declared minimum and maximum values. Its binding is read-only.
- `toggle` reads and writes a boolean binding.
- `slider` reads and writes an integer or float binding within declared minimum and maximum values. Integer bindings write integers; float bindings write floats.
- `list` presents selectable scalar values from a list or dictionary. Selection writes the selected value or dictionary key to its declared target binding.
- `repeat` instantiates a child template for each item in a list or dictionary. A list repeats in index order; a dictionary repeats in string-key order. Each instance exposes its item, zero-based index, and, for dictionaries, key to child bindings.

All bounds and bindings are validated when the project loads. A control with an invalid runtime value shows its declared fallback or stays inert; it does not coerce a string into a number or mutate the variable. Text bindings format `None`, booleans, integers, floats and strings. A list or dictionary is never implicitly stringified.

## Bindings and script values

A binding names a variable or a nested list/dictionary path, such as `gold`, `party[0].name` or `inventory["key"]`. Binding paths address existing values; screens cannot create script variables. Writable widgets update the bound value through the same variable store used by script assignments. A binding to a constant or unsupported path is a validation error.

Bindings use the script's real `Value` variants: `None`, `Bool`, `Int`, `Float`, `Str`, `List` and `Dict`. Dictionaries have string keys. `bar` and `slider` accept only `Int` or `Float`; `toggle` accepts only `Bool`; `repeat` and `list` accept `List` or `Dict`. Repeater item bindings expose the actual nested `Value`, so a row can bind to fields in a dictionary item without converting it to a display string.

Screen files do not evaluate Kairos expressions. Conditions and formatting beyond scalar interpolation are out of scope for the first version; use script labels and variables to prepare values and actions.

## Actions and script entry points

A button action is one of:

- `set`: assign a literal or another binding's current value to a writable variable path.
- `jump`: transfer control to a script label without retaining a return point.
- `call`: call a script label and resume the current script when that label returns.
- `show`: push a screen above the current screen stack.
- `hide`: remove the named screen from the stack.
- `return`: close the topmost screen and resume the interaction beneath it.

Screen actions do not execute arbitrary script. `jump` and `call` target labels, using the same label validation as script statements. A screen can be opened by script with `show screen <id>` and removed with `hide screen <id>`. Showing an already-open screen does not create a duplicate instance. `return` in a screen action closes that screen; the script statement `return` keeps its existing meaning of returning from a label.

A screen action that changes a binding updates game state before navigation takes effect. A failed action leaves the screen stack and bound values unchanged and reports a player diagnostic. The editor can expose the same action values as typed controls.

## Modality, layering and input

Screens form an ordered stack above the story view. `layer` orders screens with lower values behind higher values; the most recently shown screen wins ties. A modal screen blocks pointer and keyboard/gamepad input to screens and story controls below it. A non-modal screen leaves uncovered controls below interactive. Hiding a screen removes only that instance; `return` removes the topmost one.

Each screen has one focused widget when it contains focusable controls. Focus follows document order by default, with an optional explicit tab order. Tab and Shift-Tab move focus; arrows move among list items and directional controls. Enter or Space activates the focused control. Escape invokes the screen's configured close action when present; it does not close a screen implicitly. The player maps gamepad confirm, cancel and directional input to the same focus and activation model. Pointer activation and focus highlight remain available on both web and native players.

## Theme and save/load

Screens use the interface font and configured colors from `ui/theme.toml`, with built-in defaults when a value is absent. A widget may override a color or font asset. Screen files do not replace `ui/theme.toml`; its values remain shared by the built-in player and custom screens.

Screen layout, focus, and the visible screen stack are presentation state, not saved game state. A normal save captures the script variables and VM state; loading redraws the active story and does not restore an open custom screen. Variables needed after a new playthrough belong under `persistent.*` and persist outside save slots. Ordinary variables, including lists and dictionaries, follow the existing save-slot behavior. Screen input changes the same variables that scripts read, so no separate screen save format is needed.

## Editor surface

The editor will list files under `ui/screens/`, create and rename screens, and edit the layout tree, widget properties, bindings and actions. A canvas preview will use the player's viewport and theme. Selecting a widget will expose its typed properties and bindings; invalid values will point to the file and node. The editor writes TOML directly and preserves unknown data only if the schema later adds an explicit forward-compatibility rule. The editor is not part of this contract's initial runtime build.

## Tests

Follow the player and editor test pattern in `visual-story-editor.md`:

- **Rust parsing and validation:** valid screen files; defaults; unknown keys; duplicate IDs; missing or cyclic parents; invalid layouts; widget-specific type and range checks; unknown screens and invalid bindings with path and node diagnostics.
- **Layout and rendering:** stack order, row/column/grid measurement, fixed/fill/content sizing, anchors, clipping, viewport resizing, theme fallback and image lookup.
- **Bindings:** every supported scalar type; list and dictionary reads; nested paths; repeat order and item/key/index scope; writable toggle and slider values; rejected type mismatches; no implicit variable creation.
- **Actions and screen stack:** set before navigation; jump and call behavior; show deduplication; hide by ID; return to the underlying screen; modal input blocking; failed actions leave state unchanged.
- **Input:** pointer activation, focus traversal, keyboard activation and dismissal, list navigation, and equivalent gamepad actions.
- **Save/load:** screen presentation state is not restored; ordinary variable changes survive in save slots; persistent values survive new games and remain separate from save slots.
- **Player integration:** run the same screen fixtures in native and web players, including a screen that displays and edits a list-backed inventory.
- **Editor tests:** create and rename a screen; add and reorder nodes; edit typed properties; choose bindings and actions; round-trip TOML; report invalid nodes; preview at the target viewport.
- **End-to-end:** open a custom screen from script, change a bound value, return to the story, and verify the script reads the changed value. Repeat with save/load and a second playthrough for persistent data.

## Open questions

- Should the first release support custom transitions, or keep screen changes immediate?
- Should `list` choose an item and return immediately, or remain open until the user confirms?
- Which theme roles should custom screens inherit, and which may authors override per widget?
- Should a future editor preserve unknown TOML keys for forward compatibility, or reject them as the player does?
- Do gamepad directional controls need a configurable mapping beyond the player's default confirm, cancel and navigation actions?

# Visual story editor

Part 1 of roadmap item 3. Parts 2 (custom screens) and 3 (beginner path: templates, guided first project, code hidden until wanted) get their own specs and build on the edit engine defined here.

## Intent

Kairos targets people who want to make a visual novel without a coding background. A beginner should be able to build a short story with scenes, choices, characters, backgrounds and music without typing script. Developers keep full control: the visual tools can add, remove, select and rearrange anything, and the code editor stays one click away.

Decisions made with the owner:

- **Visual first, code optional.** Projects open in the visual tools. Both views edit the same `script/*.kai` files and show each other's changes immediately.
- **Block list plus live stage** for editing a scene.
- **Inline Code blocks** for any statement without a friendly block, editable in place, so nothing is hidden or lost.
- **Script text stays the single source of truth.** No separate visual data format.

## Current state

The story graph (`features/graph/story-graph.tsx`) shows each label as a node. Dragging between nodes inserts a `jump` through a regex edit in `shared/edits.ts`. The character form and asset drag-and-drop insert script lines the same way. Everything else needs the Monaco code editor.

## Architecture

### Rust edit engine (`kairos-script::edit`)

The script is line- and indentation-based, so a statement spans from its first line to the line before the next line at the same or lower indent. The engine uses the parser for meaning and the raw lines for exact text.

`scene_blocks(source, label) -> Vec<Block>`

- `Block { id, kind, fields, range: (first_line, last_line), children }`.
- `line` is the block's 1-based first line in the source it was read from. Every edit is applied to that exact source, so a line identifies a statement.
- `children` holds the bodies of choices and `if` branches.
- Statements without a friendly block come back as `kind: Code` with their exact text.

`apply(source, base, ops) -> Result<Applied, EditError>`

- The store detects staleness: if the buffers changed while an edit was in flight, the edit is dropped and the scene tab reloads its blocks.
- Ops:
  - Statements: `insert { after, block }`, `replace { id, block }`, `move { id, after }`, `delete { id }`.
  - Scenes: `add_scene { name, after_label }`, `rename_scene { from, to }`, `delete_scene { label, remove_references }`, `duplicate_scene { label, name }`.
  - Links: `connect { from, to, as: Jump | Choice { text } }`, `set_choice_text { id, index, text }`, `end_scene { label }`.
  - Characters: `set_character { id, name, color }`, which updates the `character` line for `id` or adds one after the last `character` line (or at the top of the file).
- `Applied { source, focus }`, where `focus` is the line of the edited or inserted block.
- Edits change only the affected lines. Comments, blank lines and unrelated lines stay byte-for-byte identical. Indentation follows the surrounding block.
- The result is reparsed before returning. An edit that adds a parse error not present before fails with `EditError::WouldBreak(message)` and changes nothing.
- Scene ops that touch other files take every project file and return every changed file: `apply_project(files, base_hashes, ops) -> Vec<(path, source)>`.

`references(files, label) -> Vec<Reference { path, line, kind: Jump | Call | Choice }>` supports delete and rename confirmations.

Scene names typed by users go through `label_from_name("Beach Day") -> "beach_day"`. It strips characters that are not letters, digits or `_`, prefixes a leading digit with `_`, and appends `_2`, `_3` and so on when the name is taken.

### napi

The addon exports `sceneBlocks`, `applyEdits`, `applyProjectEdits`, `references` and `labelFromName`. They replace `shared/edits.ts` (`insertJump`, `insertStatement`, `setCharacterLine`, `addCharacter`, `statementForAsset`) and the `ProjectEdit` IPC kinds. The current callers (graph connect, character manager, asset drop) move to the new ops.

### Editor store

- **One edit action.** Every visual change goes through `edit(ops)`. It sends the current buffers to `applyEdits`, writes the results into the buffers, pushes one undo entry, and schedules analysis. Monaco, the graph and the preview react exactly as they do to typing.
- **Stale edits.** The store detects staleness: if the buffers changed while an edit was in flight, the edit is dropped and the scene tab reloads its blocks.
- **Graph positions.** Positions saved by dragging live in `.kairos/editor.json` as `{ "graph": { "<label>": { "x": 0, "y": 0 } } }`. The engine and exporter ignore `.kairos/`. Nodes without a saved position use the automatic layout.

## Graph tools

The story graph is the home view of a project. Each node is a scene (a label).

- **Select.** Click a node to select it, shift-click or drag a box to select several, and double-click to open the scene in the block editor. A selected node shows a toolbar with Open, Rename, Duplicate and Delete.
- **Add.** An "Add scene" button, a right-click "New scene here" on empty canvas, or dragging a node's handle onto empty space. The user types a name, and `label_from_name` turns it into a label. A new scene contains one empty dialogue line and `return`. Dropping from a handle also connects the source to the new scene.
- **Connect.** Drag between nodes and choose "Go to" (a `jump`) or "Add as a choice", which prompts for choice text. If the source scene ends in a menu, the choice joins that menu; otherwise a new menu is added at the end.
- **Edit arrows.** Clicking an arrow selects it. Choice arrows show their text and rename in place. Delete removes that `jump` or menu option. On a fall-through arrow the toolbar offers "Make explicit" (insert a `jump`) and "End scene here" (insert `return`).
- **Delete scenes.** The confirmation lists the arrows that lead to the scene, such as "3 choices and 1 jump lead here", and offers to remove those links as well. Links left behind show as red arrows to a "missing scene" ghost node, and clicking the ghost offers "Create this scene".
- **Rename.** Updates every `jump` and `call` to the scene in every file. A taken name is refused.
- **Arrange.** Dragged positions persist, and "Tidy up" reruns the automatic layout. The right-click menu has "Set as start", which updates `start` in `kairos.toml`. The start scene keeps its flag.
- **Shortcuts.** Delete or Backspace deletes, F2 renames, Enter opens, Cmd+D duplicates, and Cmd+Z / Cmd+Shift+Z undo and redo.

Each action is one `edit` call and one undo step. The actions are also in the command palette.

## Block editor and stage

Opening a scene shows a tab with the block list on the left and the live stage on the right.

### Blocks

| Block | Script | Fields |
| --- | --- | --- |
| Dialogue | `e "text"` or `"text"` | character (or Narrator), text; typing `[` suggests variables |
| Background | `scene bg room` | image picker with thumbnails; optional transition |
| Show character | `show e happy at left` | character, expression (from layered sprite attributes or image names), position, optional transition |
| Hide character | `hide e` | character |
| Music / Sound | `play music "x" loop`, `play sound "x"` | asset picker, loop, fade in |
| Stop audio | `stop music` | channel, fade out |
| Pause | `pause 1.0` | seconds, or until click |
| Choice | `menu:` | options, each with text, optional "only if" condition, and either nested blocks or a target scene |
| Go to scene / Call scene | `jump x` / `call x` | scene picker |
| Set variable | `$ x = 1`, `$ x += 1`, `$ x -= 1`, toggle as `$ x = not x` | variable (pick or create), operation, value |
| If / otherwise | `if`, `elif`, `else` | condition builder: variable, comparison, value; a condition the builder cannot express shows as editable text |
| End scene | `return` | none |
| Code | anything else | the statement's text |

Blocks use plain words: "Kai says…", "Change background", "Only if…".

### Editing

- "+ Add block" between any two blocks opens a searchable menu of block names.
- Drag reorders blocks, including into and out of choice and `if` bodies.
- Each block's menu has Duplicate and Delete.
- Keyboard: up and down move the selection, Enter edits, Cmd+Enter adds a dialogue line below, and Delete removes.
- Field edits apply as you type, debounced to 300 ms, through `edit`.
- An empty scene shows "Add the first line".
- Diagnostics appear on the block whose lines they point to, in plain language.

### Stage

- Selecting a block plays the scene up to that block in the preview, through the existing `playFromLine`.
- Dropping a background asset on the stage inserts a Background block after the selection.
- Dropping a character sprite onto the left, center or right zone updates the Show block for that character before the selection, or inserts one.
- Clicking a character on the stage selects the Show block that placed it.

## Errors and undo

- **Edits never corrupt a file.** `WouldBreak` and other failures leave the buffer unchanged and show a toast with the reason.
- **Files with parse errors.** Edits still work. Blocks inside an unparseable region come back as Code blocks.
- **One history for the project.** Each entry holds the affected files' text before and after. Visual edits push one entry per action. Code-view typing pushes one entry per file after 1 s without typing. Cmd+Z in either view undoes the latest change of either kind.
- **Saving stays explicit.** Undo restores buffers, not files. Save and Save All write buffers as today, and dirty markers update.
- **Limits.** The history keeps 200 entries and resets when a project opens.

## Testing

- **Rust**
  - Round-trip tests per op: comments, blank lines and unrelated lines survive byte-for-byte.
  - The Code-block fallback is covered for every statement kind.
  - `Stale` and `WouldBreak` cases.
  - `label_from_name` cases.
  - A property test applies random insert, move and delete sequences to the example scripts and checks that the result always parses and that `scene_blocks` describes what was inserted.
- **napi.** One shape test per export.
- **Store (vitest).**
  - `edit` updates buffers and pushes undo.
  - Undo and redo across visual and code edits.
  - The stale retry.
  - Graph positions persist.
- **Playwright e2e.** One beginner flow:
  1. Create a project and add a scene from the graph.
  2. Add dialogue, a background and a choice in the block editor.
  3. Connect the choice to the new scene.
  4. Check that the preview shows the line and the code view shows the matching script.
  5. Undo, and check that both views revert.

## Out of scope

- Custom screens (part 2) and the beginner path with templates and onboarding (part 3).
- Real-time collaboration and merging edits from two editors.
- Visual editing of `define`, `default`, `character` and `transform` outside the existing character manager. These show as Code blocks when they appear inside a scene.
- Editing `ui/theme.toml` and `ui/title.toml` visually.

# Kairos

A visual novel engine with a Rust core. Scripts use a Ren'Py-flavoured syntax with a sandboxed expression language instead of Python.

```
character e "Eileen" color="#c8ffc8"
default curious = 0

label start:
    scene bg room with fade
    show e happy at center
    e "Welcome to Kairos."
    menu:
        "Tell me more":
            $ curious += 1
            jump about
        "Bye":
            return
```

## Status

The language, compiler, interpreter and graphical player work, natively and in the browser. The Electron editor (`apps/editor`) edits projects with live diagnostics, a story-flow graph, a live preview and a terminal. The AI writing assistant and Ren'Py import are not built yet.

| Crate | Role |
| --- | --- |
| `kairos-script` | Parser for `.kai` files, with diagnostics that keep parsing past errors |
| `kairos-vm` | Compiler to a flat instruction list, interpreter, rollback, saves |
| `kairos-project` | Project loading, checks, export assembly and the asset naming rule |
| `kairos-render` | wgpu renderer: scene, text (glyphon), fade and dissolve transitions |
| `kairos-audio` | kira mixer with `music`, `sound` and `voice` channels |
| `kairos-player` | Window or browser canvas with the default game UI |
| `kairos-cli` | `kairos check`, `kairos translate`, `kairos run` and `kairos export` |
| `kairos-napi` | Node bindings for the editor: diagnostics, outline, story graph |

Saves point at statements by a stable ID (label, source-text hash and occurrence), so a save keeps working after edits that leave the saved line and its call sites alone.

## Try it

```sh
cargo run -p kairos-cli -- run examples/hello-kairos
cargo run -p kairos-cli -- check examples/hello-kairos
cargo run -p kairos-cli -- translate examples/hello-kairos --lang ja
cargo run -p kairos-cli -- export examples/hello-kairos --target web --out /tmp/hello-kairos-web
cargo run -p kairos-cli -- export examples/hello-kairos --target desktop --out /tmp/hello-kairos-desktop
cargo run -p kairos-cli -- run --headless examples/hello-kairos --inputs examples/hello-kairos/inputs.txt
cargo test --workspace
```

## Editor

```sh
bun install                              # also downloads the Electron binary
bun run build:napi                       # builds crates/kairos-napi for the editor
scripts/build-wasm.sh                    # builds the preview player into crates/kairos-player/pkg
bun run dev                              # rebuilds the napi binding if Rust changed, then opens the editor
KAIROS_PROJECT=$PWD/examples/hello-kairos bun run dev   # opens a project directly
bun run test && bun run typecheck
bun run --cwd apps/editor package         # builds an unsigned Kairos.app on macOS
bun run --cwd apps/editor test:e2e       # Playwright Electron smoke test
```

Projects open on the story flow. Add, rename, duplicate and delete scenes there; drag between scenes to add a jump or a choice; click an arrow to edit or remove it. Double-click a scene to edit it as blocks: dialogue, backgrounds, characters, music, choices, conditions and variables, with the preview showing the selected moment. Anything without a block shows as a Code block you can still edit. Visual edits and typing share one undo history (Cmd+Z), and both change the same script files.

The preview pane loads the WebAssembly player from `crates/kairos-player/pkg` and shows build instructions when it is missing.

## Export

`kairos export <project> --target web|desktop --out <directory>` checks the project, then builds a web folder or a desktop bundle for the host platform. Web export includes `index.html`, the wasm player, `game.json` and project assets; run it from any static host. Build the web player first with `scripts/build-wasm.sh`, or pass its package directory with `--player-pkg`. Desktop export copies the running `kairos` executable; double-clicking the exported app starts the project bundled beside it. The editor exposes the same targets through the command palette's **Export game** command.

## Playing

Click, space or enter advances; the first press finishes the typewriter. Number keys or clicks pick menu choices. Mouse wheel up or PageUp rolls back. Escape or right-click opens the menu with Save, Load, History, Settings (text speed, auto delay, volumes, skip and language), Auto, Skip, Restart and Main menu. `A` toggles auto, Tab toggles skip. Skip stops at lines you have not read yet, in this playthrough or an earlier one; set Skip to All text in Settings to run through everything.

Saves go to the platform data directory, e.g. `~/Library/Application Support/Kairos/saves/<project name>/slot-N.ksav` on macOS. In the browser they go to `localStorage`. The selected language is stored in player preferences, separately from save files.

If an update rewords or removes the line a save points to, loading resumes after the nearest earlier line that still exists in the same label and says so. A save fails to load only when its whole label is gone.

### Persistent data

Variables named `persistent.*` belong to the player, not to a playthrough. They keep their values across new games, loads and rollback, and they are stored next to the save slots in `persistent.kper` (or `localStorage` in the browser), together with the record of read lines. Use them for galleries, endings and route locks.

```
default persistent.endings_seen = 0

label good_end:
    $ persistent.endings_seen += 1
    $ persistent.true_route_open = True
    return

label start:
    if persistent.true_route_open:
        jump true_route
```

`default` sets a starting value only the first time the game runs. A `persistent.*` name that was never set reads as `None`.

### Lists, dictionaries and random numbers

Variables can hold lists and dictionaries. Dictionary keys are strings.

```
default inventory = []
default affection = {"kai": 0, "eileen": 0}

label start:
    $ inventory.append("map")
    $ affection["kai"] += randint(1, 3)
    if "map" in inventory and affection["kai"] > 2:
        "You have [len(inventory)] items. The first is [inventory[0]]."
    $ ending = choice(["rain", "sun"])
```

- Read with `xs[0]`, `xs[-1]`, `d["key"]`, `x in xs`, `"key" in d`, `x not in xs`, and `+` to join lists.
- Change in place with `$ xs.append(v)`, `extend(list)`, `insert(i, v)`, `remove(v)`, `pop()`, `pop(i)`, `clear()`, `$ d.pop("key")`, and assignment such as `$ d["a"]["b"] = 1`.
- Read-only methods: `d.get("key", default)`, `d.keys()`, `d.values()`, `xs.count(v)`, `xs.index(v)`.
- Functions: `len`, `min`, `max`, `abs`, `int`, `float`, `str`, `round`, `range`, `random()` (0 to 1), `randint(a, b)` (inclusive) and `choice(list)`.
- `[...]` in dialogue takes any expression: `"[affection['kai'] + 1]"`.

Each new game gets its own random seed. Rollback and loading replay the same draws, so a player cannot reroll a result by stepping back. `kairos check` reports unknown functions and methods.

## Project layout

```
kairos.toml        name, start label, optional [window] width/height (default 1280x720)
script/*.kai       story scripts, compiled in file-name order
assets/images/     backgrounds and sprites
assets/audio/      music, sounds, voice
ui/theme.toml      optional game UI styling
ui/title.toml      optional title screen styling
tl/<lang>.toml     optional translations, generated by `kairos translate`
```

### Localization

`kairos translate <project> --lang ja` creates or updates `tl/ja.toml`. It preserves translations for current IDs, adds empty values with source comments, and moves removed IDs under `[stale]`. `kairos check` warns while a translation file has stale entries. Dialogue, menu captions and choices, and character names use stable statement IDs; choice keys also include their zero-based choice index. Empty entries fall back to the source string, then `[var]` interpolation runs on the selected string.

```toml
# source: Welcome, [name].
"start:4d89db3b15f7e2a1:0" = ""
"start:4d89db3b15f7e2a1:1#choice:0" = ""
```

The Settings screen lists Source and the available `tl/*.toml` languages. Changing language does not change save data.

### Asset names

The editor resolves assets with the same rule, so treat it as stable.

- An image name is its file stem anywhere under `assets/images/`. `scene bg room` can load `assets/images/bg room.png` or `assets/images/bg/bg room.webp`; extensions are tried in `.png`, `.jpg`, `.jpeg`, `.webp` order within each directory. Duplicate image stems in different directories fail `kairos check`.
- Audio names with `/` resolve under `assets/audio/` as written. A bare name searches recursively: `theme.ogg` matches that filename, while `theme` matches the stem using `.ogg`, `.mp3`, `.wav` and `.flac` order. Duplicate audio names in different directories fail `kairos check`.
- A character with `assets/images/sprites/<id>/layers.toml` uses its layered sprite definition instead of a flat image. The file lists groups from back to front; `show e happy casual` selects those attribute layers, then group defaults or a group's only image. Unknown attributes fail `kairos check`.
- `assets/audio/audio.toml` can define loop points by resolved path relative to `assets/audio/`. The intro before `loop_start` plays once; the loop region then repeats.
- `[audio]` in `kairos.toml` can duck music while voice plays: `duck` multiplies music volume and defaults to `1.0` (off); `duck_fade` sets the transition time in seconds and defaults to `0.25`.
- Playing a new music track with `fadein` crossfades the current music over the same duration.
- A missing image draws a labelled placeholder. A missing sound is skipped with a warning.

Backgrounds are scaled to cover the screen. Sprites are scaled to the screen height and stand on the bottom edge, at `left`, `center` (the default) or `right`. `fade` goes through black over 1 s, `dissolve` crossfades over 0.5 s, and `move` currently crossfades too.

### Title flow

`label splashscreen` runs before the title screen; returning or reaching its end continues to the title. `label main_menu` replaces the built-in title and can use `jump start` to begin the story. `open load`, `open settings` and `quit` are available in script labels. The in-game menu's Main menu button returns to the title without replaying the splashscreen.

Every key in `ui/title.toml` is optional:

```toml
title = "Hello Kairos"        # defaults to the kairos.toml name
background = "bg title"       # image name
logo = "logo"                 # image name, replaces title text
music = "title.ogg"           # audio name, loops
buttons = ["start", "continue", "load", "settings", "quit"]
```

Continue loads the newest save and stays hidden until a save exists. `start` begins at the manifest's start label. Saving stays disabled while `splashscreen` or `main_menu` runs.

### Theme

Every key in `ui/theme.toml` is optional:

```toml
[fonts]
dialogue = "fonts/MySerif.ttf"   # relative to assets/
interface = "fonts/MySans.ttf"

[textbox]
color = "#1b1b2b"
color_bottom = "#0d0d16"         # set for a vertical gradient
opacity = 0.88
height = 190
margin = 24
padding = 28
radius = 14
text_color = "#f2f2f7"
text_size = 28

[nameplate]                      # color, color_bottom, opacity, text_color, text_size, radius
[choice]                         # color, color_bottom, opacity, hover_color, text_color, text_size, width, height, spacing, radius
[menu]                           # color, color_bottom, opacity, accent, text_color
```

The nameplate text uses the character's `color` unless `text_color` is set. The bundled font is Lato (SIL Open Font License, `crates/kairos-render/fonts/OFL.txt`).

## Browser build

```sh
scripts/build-wasm.sh            # writes crates/kairos-player/pkg/ (gitignored)
```

The script installs the `wasm32-unknown-unknown` target and a `wasm-bindgen-cli` that matches `Cargo.lock`. The package is an ES module for `wasm-bindgen --target web`:

```ts
import init, { WebPlayer } from "./pkg/kairos_player.js";
await init();
const player = new WebPlayer(canvas);
await player.ready();            // WebGPU, or WebGL2 where WebGPU is unavailable
player.load(JSON.stringify({
  manifest: kairosTomlText,
  scripts: [{ path: "script/a.kai", source }],
  translations: [{ lang: "ja", source: translationTomlText }],
  assetBaseUrl: "kairos-asset://project/",   // points at the project's assets/ directory
  assets: ["assets/images/bg room.png", "assets/audio/theme.ogg"], // paths relative to the project
  theme: themeTomlText,                       // optional, contents of ui/theme.toml
  title: titleTomlText,                       // optional, contents of ui/title.toml
}));                             // throws Error with "path:line: message" lines
player.start();                  // or player.playFromLine("script/a.kai", 12)
player.resize(cssWidth, cssHeight);
player.destroy();
```

Assets are fetched from `assetBaseUrl` plus the paths above, URL-encoded (`images/bg%20room.png`). Audio starts on the first click or key press, since browsers block sound before a user gesture. `resize` takes CSS pixels and scales the drawing buffer by `devicePixelRatio`.

## License

MIT. The bundled Lato font is under the SIL Open Font License 1.1.

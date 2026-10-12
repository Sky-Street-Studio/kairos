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
| `kairos-project` | `kairos.toml`, script loading and the asset naming rule |
| `kairos-render` | wgpu renderer: scene, text (glyphon), fade and dissolve transitions |
| `kairos-audio` | kira mixer with `music`, `sound` and `voice` channels |
| `kairos-player` | Window or browser canvas with the default game UI |
| `kairos-cli` | `kairos check` and `kairos run` |
| `kairos-napi` | Node bindings for the editor: diagnostics, outline, story graph |

Saves point at statements by a stable ID (label, source-text hash and occurrence), so a save keeps working after edits that leave the saved line and its call sites alone.

## Try it

```sh
cargo run -p kairos-cli -- run examples/hello-kairos
cargo run -p kairos-cli -- check examples/hello-kairos
cargo run -p kairos-cli -- run --headless examples/hello-kairos --inputs examples/hello-kairos/inputs.txt
cargo test --workspace
```

## Editor

```sh
bun install
bun run build:napi                       # builds crates/kairos-napi for the editor
bun run dev                              # opens the Electron editor
KAIROS_PROJECT=$PWD/examples/hello-kairos bun run dev   # opens a project directly
bun run test && bun run typecheck
bun run --cwd apps/editor test:e2e       # Playwright Electron smoke test
```

The preview pane loads the WebAssembly player from `crates/kairos-player/pkg` and shows build instructions when it is missing.

## Playing

Click, space or enter advances; the first press finishes the typewriter. Number keys or clicks pick menu choices. Mouse wheel up or PageUp rolls back. Escape or right-click opens the menu with Save, Load, History, Settings (text speed, auto delay, volumes), Auto, Skip and Restart. `A` toggles auto, Tab toggles skip.

Saves go to the platform data directory, e.g. `~/Library/Application Support/Kairos/saves/<project name>/slot-N.ksav` on macOS. In the browser they go to `localStorage`.

## Project layout

```
kairos.toml        name, start label, optional [window] width/height (default 1280x720)
script/*.kai       story scripts, compiled in file-name order
assets/images/     backgrounds and sprites
assets/audio/      music, sounds, voice
ui/theme.toml      optional game UI styling
```

### Asset names

The editor resolves assets with the same rule, so treat it as stable.

- An image name is its file stem anywhere under `assets/images/`. `scene bg room` can load `assets/images/bg room.png` or `assets/images/bg/bg room.webp`; extensions are tried in `.png`, `.jpg`, `.jpeg`, `.webp` order within each directory. Duplicate image stems in different directories fail `kairos check`.
- Audio names with `/` resolve under `assets/audio/` as written. A bare name searches recursively: `theme.ogg` matches that filename, while `theme` matches the stem using `.ogg`, `.mp3`, `.wav` and `.flac` order. Duplicate audio names in different directories fail `kairos check`.
- A character with `assets/images/sprites/<id>/layers.toml` uses its layered sprite definition instead of a flat image. The file lists groups from back to front; `show e happy casual` selects those attribute layers, then group defaults or a group's only image. Unknown attributes fail `kairos check`.
- A missing image draws a labelled placeholder. A missing sound is skipped with a warning.

Backgrounds are scaled to cover the screen. Sprites are scaled to the screen height and stand on the bottom edge, at `left`, `center` (the default) or `right`. `fade` goes through black over 1 s, `dissolve` crossfades over 0.5 s, and `move` currently crossfades too.

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
  assetBaseUrl: "kairos-asset://project/",   // points at the project's assets/ directory
  assets: ["assets/images/bg room.png", "assets/audio/theme.ogg"], // paths relative to the project
  theme: themeTomlText,                       // optional, contents of ui/theme.toml
}));                             // throws Error with "path:line: message" lines
player.start();                  // or player.playFromLine("script/a.kai", 12)
player.resize(cssWidth, cssHeight);
player.destroy();
```

Assets are fetched from `assetBaseUrl` plus the paths above, URL-encoded (`images/bg%20room.png`). Audio starts on the first click or key press, since browsers block sound before a user gesture. `resize` takes CSS pixels and scales the drawing buffer by `devicePixelRatio`.

## License

MIT. The bundled Lato font is under the SIL Open Font License 1.1.

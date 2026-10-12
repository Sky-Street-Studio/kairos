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

The language, compiler and interpreter work headless, and the Electron editor (`apps/editor`) edits projects with live diagnostics, a story-flow graph and a terminal. The AI writing assistant and Ren'Py import are not built yet.

| Crate | Role |
| --- | --- |
| `kairos-script` | Parser for `.kai` files, with diagnostics that keep parsing past errors |
| `kairos-vm` | Compiler to a flat instruction list, interpreter, rollback, saves |
| `kairos-cli` | `kairos check` and `kairos run --headless` |
| `kairos-napi` | Node bindings for the editor: diagnostics, outline, story graph |

Saves point at statements by a stable ID (label, source-text hash and occurrence), so a save keeps working after edits that leave the saved line and its call sites alone.

## Try it

```sh
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

## Project layout

```
kairos.toml        name, start label
script/*.kai       story scripts, compiled in file-name order
assets/images/     `scene bg room` loads assets/images/bg room.{png,jpg,jpeg,webp}
assets/audio/      `play music "theme.ogg"` loads assets/audio/theme.ogg
ui/                interface files
```

## License

MIT

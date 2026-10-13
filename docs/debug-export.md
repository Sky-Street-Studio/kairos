# Debugger and export contract

Shared rules for the debugger, game export and editor packaging lanes.

## Debugger

The web player exposes a read-mostly debug surface to the editor. All values cross as JSON strings.

| Method | Returns / effect |
| --- | --- |
| `debugState()` | `{ "label": string, "path": string, "line": number, "variables": { name: value }, "callStack": [{ "label": string, "path": string, "line": number }], "history": number, "position": number }` |
| `setVariable(name, json)` | Sets a `default`/`define`d variable or a `$` variable already in the store. Unknown names are an error. Takes effect from the next statement. A `define` override is a debug-session setting: it is never saved and survives rollback until the preview reloads. |
| `rollbackTo(position)` | Rolls back to an earlier history position (`0..history`). Uses the existing rollback; forward moves are not supported. |
| `playFromLine(path, line, overrides)` | Existing call gains an optional overrides JSON object applied before the first statement runs. Unlike `setVariable`, overrides may create `$` variables the fresh VM has not seen yet. |

The player calls `onDebug` (a JS callback passed at load) after every statement that changes the stage, variables or position. It is called at most once per frame.

Values are the VM's scalar values: numbers, strings, booleans and `None` as `null`. JSON arrays and objects are rejected with an error naming the variable.

The editor shows a Debug tab in the bottom panel: a variables table (editable cells call `setVariable`), the call stack (click jumps the editor to that line), and a history slider bound to `rollbackTo`. The play-from-here button opens a small overrides form prefilled with the current variables.

## Game export

`kairos export <project> --target web|desktop --out <dir>` and the editor's Export command produce the same output. The file assembly lives in `kairos-project` so the CLI and napi share it.

- **web:** `<out>/index.html`, the wasm player files, `game.json` (manifest, scripts, `ui/theme.toml`, `ui/title.toml`, `tl/*.toml`, and the asset path list, in the shape the editor already sends to the player) and `assets/`. It runs from any static host. The player files are found at `crates/kairos-player/pkg` in development, or at a path passed with `--player-pkg`.
- **desktop:** a build for the host platform only. On macOS, `<out>/<Name>.app` with `Contents/MacOS/<name>` (the player binary), `Contents/Resources/game/` (the project files listed below) and an `Info.plist` whose `CFBundleName` is the manifest name. On Linux and Windows, a folder with the binary and `game/`. The player, started with no arguments, runs `game/` next to its executable (or `../Resources/game` on macOS).
- Export copies only `kairos.toml`, `script/`, `assets/`, `ui/`, `tl/`. It runs `kairos check` first and refuses to export a project with errors.

## Editor packaging

`bun run --cwd apps/editor package` builds `Kairos.app` with electron-builder: product name Kairos, app id `studio.skystreet.kairos`, the Kairos mark as the icon, the napi binding and the wasm player included as extra resources. In the packaged app the player and native binding load from `process.resourcesPath`. Unsigned builds are fine; signing and notarization are out of scope.

# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The editor is an Electron app. Its UI is a React renderer, so web conventions apply, inside a desktop window on macOS first.

## Users

- Solo and hobby creators who write, stage and ship a visual novel alone, many of them coming from Ren'Py.
- Small VN studios where writers, artists and a programmer share one project and ship commercial games, Steam first.
- Later, established studios, such as Blits Game and Yangyang Mobile.

The roadmap measures readiness by whether a studio could ship a game the size of Camp Buddy or Steins;Gate.

## Product Purpose

Kairos is a visual novel engine and editor with a Rust core. Creators write scripts in a Ren'Py-flavoured language. They edit with live diagnostics, a story-flow graph, a live preview, drag-and-drop staging and an embedded terminal, then export to the web and desktop. It succeeds when a team can take a game from first line to a store release without leaving Kairos or writing engine code.

## Positioning

Kairos differs from Ren'Py on four points:

- **A modern editor.** It is an IDE built around the story: the preview runs beside the script, the flow graph edits jumps, and assets are staged by dragging them in. A visual story editor is in progress.
- **A Rust runtime.** The wgpu player is the same crate natively and in the browser. That opens web, desktop, and later mobile and console ports, which Ren'Py struggles with.
- **Writes like Ren'Py.** The syntax is familiar, and a sandboxed expression language replaces embedded Python. An import path for Ren'Py projects is planned.
- **AI that helps you write.** It helps with story and code through the creator's own model access, inside the editor.

## Operating Context

- **Projects** are folders: `kairos.toml`, `.kai` scripts under `script/`, assets under `assets/` with folder categories, and UI config under `ui/`.
- **Collaboration** happens through git and ordinary file tools outside the editor.
- **CLI agents** (Claude Code, Codex) run in the editor's embedded terminal against the same files.
- **The command line** covers the same work: `kairos check`, `translate`, `run` and `export`.

## Capabilities and Constraints

The runtime and editor provide:

- **Runtime:** rollback, skip that stops at unread text, auto mode, save slots that survive edits, persistent data, layered sprites, localization keyed by stable line IDs, a debugger, and lists, dictionaries, built-in functions and seeded random numbers in scripts.
- **Editor:** script editing with diagnostics, the story-flow graph, live preview, an asset manager, character management, a theme editor, and previews for images, audio and project text files.
- **Export:** web and macOS.

Planned, in roadmap order:

- custom screens
- Windows and Linux export, signing and Steam integration
- text tags, NVL mode and fonts for Chinese, Japanese and Korean
- video, asset packing, a route tester, a voice workflow, accessibility, mobile and console ports, and mods

Undecided: the scope of AI assistance beyond writing and coding help.

## Brand Commitments

- **Open source.** Kairos is open source under the MIT license, in the public Sky-Street-Studio/kairos repository.
- **Bring your own AI.** AI connects only through the creator's own Anthropic or OpenAI keys, or through Claude Code and Codex in the terminal. There is no Kairos-hosted AI service.
- **Projects are plain files.** A project stays a readable folder that works in git and outside the editor. The editor never locks content into a private format.

## Evidence on Hand

- `examples/hello-kairos`: a working sample project with scripts, backgrounds, layered sprites and music.
- `docs/roadmap.md`, `docs/creator-toolkit.md` and `docs/debug-export.md`: the plan and the engine contracts.
- There are no testimonials, user counts, shipped games or studio partnerships yet. Do not claim any.

## Product Principles

1. The script is the source of truth. Visual tools read and write it, and never hold state the script can't express.
2. Ship-readiness drives priority. Build what blocks a real release before what demos well.
3. Familiar to Ren'Py users, without Ren'Py's limits. Keep the syntax recognisable and the runtime portable.
4. Stay open. Plain files, the MIT license and bring-your-own AI keep creators free to leave with their work.

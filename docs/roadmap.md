# Roadmap

What Kairos needs before a studio can ship a game the size of Camp Buddy or Steins;Gate with it. Items are in build order. Each one lists what it unlocks, so you can see why it sits where it does.

## Where the engine stands

The player has rollback, skip, auto mode, text speed, save slots, layered sprites, localization keyed by stable line IDs and a debugger. It exports to the web and to macOS. The VM holds scalar values only (none, bool, int, float, string).

## 1. Persistent data, seen lines and save migration

A `persistent` store that survives across playthroughs and lives outside save slots, plus a record of every line ID the player has seen.

- Unlocks CG galleries, endings lists, route locks (a true route that opens after the other endings) and replay menus.
- Lets skip stop at unread text. Skip passes over every line today.
- Save migration ships with it, because this item and the next both change what a save holds. `SaveData` already carries a version number, but postcard cannot read an older layout, so each version bump needs an explicit upgrade from the previous one. A save also fails with `MissingStatement` when a patch rewords the saved line. Fix that by falling back to the nearest surviving statement in the same label, and report the fallback to the player.

## 2. Script data types

Lists, dictionaries, functions and a seeded random source in the script language and VM.

- Unlocks affection and stat systems, inventories, unlockable glossaries (Steins;Gate's TIPS) and randomized events.
- Random draws must replay identically under rollback, so the seed belongs in VM state and in saves.

## 3a. Visual story editor (done)

Build and edit a branching story from the visual flow and block editors, with the script available for direct edits. Design: `visual-story-editor.md`.

## 3b. Custom screens

A declarative way to build interactive UI, with layout, buttons, text and images bound to script variables. Today the customization surface is `ui/theme.toml` and `ui/title.toml`.

- Unlocks phone and messaging systems, stat screens, maps, custom save/load and settings screens, and simple minigames.
- This item needs the most design work. Write a contract doc before building it, the same way `creator-toolkit.md` came first.

## 3c. Beginner path

Templates, a guided first project and code hidden until wanted.

Items 3 and 4 can swap. Take export first if you want a game on Steam early. Take screens first if the next milestone is a demo that shows off custom UI.

## 4. Desktop export and stores

- Windows and Linux builds of the player and the export pipeline.
- macOS signing and notarization.
- Steam integration: achievements and cloud saves. Achievements read from the persistent store in item 1.

## 5. Text and staging

- Text tags: bold, italic, color, size, inline waits, shake and ruby text for Japanese furigana.
- NVL mode (full-screen narration).
- CJK font fallback.
- Then the deferred staging work: transform timeline, camera, screen effects (shake, flash) and shaders.

## Next in line

Ordered by how often they block a release:

- **Video playback.** Opening movies and animated cutscenes.
- **Asset streaming and packing.** Load and evict images on demand, and pack assets into archives (optionally encrypted) instead of shipping loose files.
- **Route testing.** A tool that plays every branch and reports dead ends, unreachable labels and missing assets.
- **Voice workflow.** Voice files resolved by line ID, per-character volume and voice replay from the history screen.
- **Accessibility.** Controller input, text-to-speech, font scaling.
- **Mobile and console ports.** The wgpu player can target iOS, Android and Switch, which Ren'Py struggles with.
- **Mod support.**

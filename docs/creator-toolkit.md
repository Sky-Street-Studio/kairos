# Creator toolkit contract

Shared rules for the asset, title, audio and localization work. Lanes are built in parallel, so anything one lane relies on from another is fixed here. Change this file before changing a rule.

## Asset lookup

Paths are relative to `assets/`.

- An image name is a file stem anywhere under `images/`. `scene bg room` finds `images/bg room.png`, `images/bg/bg room.png` or `images/anything/deeper/bg room.webp`. Within one directory the extension order stays png, jpg, jpeg, webp.
- The same image stem in two directories is a `kairos check` error naming both paths. Same stem with different extensions in one directory keeps the extension order and is not an error.
- An audio name containing `/` resolves under `audio/` exactly as today. A bare name searches all of `audio/`: `"theme.ogg"` matches the file name, `"theme"` matches the stem using the extension order ogg, mp3, wav, flac. Duplicates are a `kairos check` error.
- Audio formats are ogg, mp3, wav, flac in both the engine and the editor.
- The web player cannot list directories. The editor passes the full list of asset paths when it starts the player, and the player builds the same index from it.

## Folder categories

Used by the editor only; the engine ignores categories.

| Folder | Category |
| --- | --- |
| `images/bg/` | Background |
| `images/sprites/<id>/` | Sprite of character `<id>` |
| `images/ui/` | UI (title backgrounds, logos) |
| `audio/music/`, `audio/sfx/`, `audio/voice/` | Music, Sound, Voice |

Files elsewhere fall back by name: an image stem starting with `bg ` is a background, an image whose first word is a declared character id is that character's sprite, anything else is Other.

## Layered sprites

A character is layered when `images/sprites/<id>/layers.toml` exists:

```toml
order = ["base", "outfit", "face"]   # back to front

[default]
outfit = "uniform"
face = "neutral"
```

Layer files live at `images/sprites/<id>/<group>/<attribute>.<ext>`. `show e happy casual` treats `e` as the tag and the rest as attributes. For each group in `order`, draw the file whose stem matches an attribute, else the group's default, else the group's only file, else nothing. An attribute that matches no group is a `kairos check` error. Layered and flat lookup never mix for one tag: if `layers.toml` exists, flat `e happy.png` is ignored.

## Audio metadata

`assets/audio/audio.toml`, keyed by path relative to `assets/audio/`:

```toml
["music/theme.ogg"]
loop_start = 12.48   # seconds; the part before plays once
loop_end = 96.0      # optional, defaults to end of file
```

`kairos.toml` gains an optional `[audio]` table: `duck = 0.5` multiplies music volume while voice plays (default 1.0, off), `duck_fade = 0.25` seconds. `play music "x" fadein 1.0` while music is playing crossfades: the old track fades out over the same time.

## Title flow

1. If `label splashscreen` exists it runs first. Its `return` (or falling off the end) goes to the title.
2. If `label main_menu` exists it runs as the title. Otherwise the built-in title screen draws from `ui/title.toml`.
3. `jump <label>` from `main_menu` starts the game. New statements usable anywhere: `open load`, `open settings`, `quit` (a no-op where the host cannot quit).
4. Saving is disabled during `splashscreen` and `main_menu`.
5. The in-game menu gains "Main menu", which returns to step 2.

```toml
# ui/title.toml, every key optional
title = "Hello Kairos"        # defaults to kairos.toml name
background = "bg title"       # image name
logo = "logo"                 # image name, drawn instead of the title text
music = "title.ogg"           # audio name, loops
buttons = ["start", "continue", "load", "settings", "quit"]
```

`continue` loads the newest save and is hidden when there is none. `start` jumps to the manifest's `start` label.

## Localization

- Translatable strings: dialogue and narration lines, menu choice text, character display names.
- Each entry is keyed by the existing stable statement ID used by saves (label, source hash, occurrence). Choice text adds the choice index.
- `kairos translate <project> --lang ja` creates or updates `tl/ja.toml`, keeping existing translations, adding new keys with empty text and the source as a comment, and moving entries whose ID vanished under a `[stale]` table.
- An empty or missing translation falls back to the source text. `[var]` interpolation runs after the lookup.
- The player's Settings screen lists the source language plus each `tl/*.toml`. The choice is saved with the player preferences, not in save files.

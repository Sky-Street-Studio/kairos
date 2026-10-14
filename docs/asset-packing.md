# Asset packing and streaming

`kairos export` can ship assets as loose files or as one `assets.kpak` archive. The editor export dialog has the same option. Without `--pack-assets`, exports keep their existing loose-file layout. The editor preview always reads the project's loose `assets/` folder.

```sh
kairos export ./my-game --target desktop --out ./build --pack-assets
kairos export ./my-game --target web --out ./site --pack-assets
```

For script and game configuration, `kairos.toml` remains in the bundle. Desktop players prefer `assets.kpak` beside that manifest and otherwise read loose files under `assets/`. The web player fetches assets when the script reaches them; a packed web export fetches and caches the archive on its first asset request. Static hosts need to serve `.kpak` files with the same origin rules as `game.json`.

## Archive format

A `.kpak` begins with the eight bytes `KPAK0001`, followed by a little-endian `u32` entry count. Each index entry contains a little-endian `u32` UTF-8 path length, the path bytes relative to `assets/`, a little-endian `u64` data offset from the start of the archive, and a little-endian `u64` byte length. File payloads follow the complete index in path order. The format has no compression or integrity check; source assets are stored as-is.

Asset paths remain visible in the index. To obfuscate payloads, set a non-empty key in the project manifest:

```toml
[assets]
packing_key = "replace-with-your-project-key"
```

Kairos applies repeating-key XOR to each payload. The player reads the same key from the bundled manifest. This is obfuscation, not encryption or DRM: the key ships with the game, the archive index is readable, and a player can recover the original bytes. Do not use it to protect secrets or third-party content you are not allowed to distribute.

## Texture memory

The native player requests images used by the current stage and prefetches image references in the next 20 VM instructions. It tracks uploaded RGBA texture sizes and evicts the least-recently-used images that are not on the current stage when usage exceeds the configured budget. An evicted image is loaded again if the story returns to it. Audio, save data, rollback history and editor preview files are outside this texture budget.

```toml
[assets]
memory_budget_mb = 256
```

The default is 256 MiB. Values below 1 are treated as 1 MiB. The budget covers GPU image textures estimated at four bytes per pixel; decoding buffers, audio, renderer snapshots and driver allocations are not included. The active stage is retained even when its textures alone exceed the budget.

Loose and packed exports use the same script asset lookup rules. Packing does not alter project files, saves, rollback state or the editor's loose-file preview. Re-export after changing assets or the packing key so the archive matches the manifest.

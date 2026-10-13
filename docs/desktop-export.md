# Desktop export contract

`kairos export <project> --target <target> --out <dir>` checks the project, copies its game files and places a Kairos player in the output folder. The editor uses the same exporter.

The exporter copies `kairos.toml`, `script/`, `assets/`, `ui/` and `tl/`. It rejects output paths inside the project or its copied folders.

| Target | Output |
| --- | --- |
| `web` | Web player files, `game.json`, `index.html` and project assets |
| `desktop` | Host-platform desktop build |
| `windows-x64` | `<name>.exe`, `game/` and `run.bat` |
| `linux-x64` | `<name>`, `game/` and executable `run.sh` |
| `macos` | `<Name>.app` with the player in `Contents/MacOS` and game files in `Contents/Resources/game` |

Windows filesystems do not keep Unix permissions, so a Linux export made on Windows needs `chmod +x run.sh <name>` before it runs. Ship it in a tar archive created on Linux or macOS to keep the bits.

The Linux and Windows launchers start the executable from the output directory, so the player can find the adjacent `game/` folder. The macOS player loads `../Resources/game`. The CLI uses its own executable as the host player for `desktop`; pass `--player-binary <path>` for an externally built target player. The editor uses its packaged or development host player, or `KAIROS_PLAYER_BINARY_WINDOWS_X64`, `KAIROS_PLAYER_BINARY_LINUX_X64` or `KAIROS_PLAYER_BINARY_MACOS` for a supplied target binary. Kairos does not cross-compile the player. Without a matching binary, cross-target export fails with a host-only error. Build the player with the Rust target toolchain you have installed, then supply that binary.

## macOS signing

Unsigned `.app` exports work without Apple credentials. The exporter prints a warning and skips signing and notarization when these variables are absent:

- `KAIROS_MACOS_SIGNING_IDENTITY`: codesign identity
- `KAIROS_MACOS_TEAM_ID`: Apple developer team ID
- `KAIROS_MACOS_NOTARY_PROFILE`: `notarytool` keychain profile

Set all three to run `codesign`, submit with `xcrun notarytool --wait`, then staple the ticket with `xcrun stapler`. A partial configuration returns an error. The export process never creates or stores credentials.

## Steam

Steam support is optional and disabled in the default player build. Enable it with `cargo build -p kairos-player --features steam`. The feature requires the Steamworks SDK expected by `steamworks`; CI does not commit or package SDK redistributables.

Add Steam configuration to `kairos.toml`:

```toml
[steam]
app_id = 123456

[steam.achievements]
ACH_FIRST_ENDING = "persistent.endings"
ACH_GALLERY = "persistent.gallery_unlocked"
```

Achievement keys are Steam API names; each value names a key in the persistent store. The player unlocks an achievement when that stored value is a true boolean, nonzero number or nonempty string. Define achievements in Steamworks with matching API names and configure the app ID before launch.

With the `steam` feature and config enabled, the player opens Steam user stats, unlocks matching achievements and stores the updated stats. It mirrors files under the project's Kairos save directory to Steam Cloud, downloading newer cloud files and uploading newer local files. Steam Cloud must be enabled for the app and account. If Steam is not running, initialization warns and the game continues without achievements or cloud sync. No Steam config also means no Steam initialization.

## CI artifacts

The Rust CI matrix builds the CLI player on Linux, macOS and Windows and uploads each platform's release executable as a workflow artifact named `kairos-player-<OS>`. Artifacts expire after 14 days. Download the matching executable and provide it to the exporter for cross-target builds.

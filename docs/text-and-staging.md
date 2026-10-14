# Text and staging contract

Text and stage state belong to the script and VM, not to a platform-specific player. Both native and web players use the same rendering and playback rules.

## Implementation status

Bold, italic, color, size, inline waits and small inline ruby readings use the shared native/web text renderer; ruby positioning and animated text shake remain. NVL on/off/clear, the character `nvl` flag, full-screen accumulated pages, backlog, rollback, and save/load are implemented; save version 4 upgrades version 3 saves. Theme fallback font paths load into the shared font database, native players also try installed system CJK fonts, and the renderer wraps CJK runs. Sprite/camera timelines and screen shake/flash run from saved stage state and snap under skip. Per-image shaders are deferred.

## Text

Dialogue supports `{b}`, `{i}`, `{color=#rrggbb}`, `{size=N}`, `{w=seconds}`, `{shake}`, `{/b}`, `{/i}`, `{/color}`, `{/size}`, `{/shake}`, and `{rb}base{/rb}{rt}reading{/rt}`. Style tags nest and affect only their enclosed text. Ruby keeps its base and reading together. Wait and shake tags do not display; waits pause typewriter reveal at their position, while shake animation remains unimplemented. Tags remain in source and translation values, while statement IDs hash markup-normalized source so markup-only edits preserve existing IDs. Markup-free lines keep their existing IDs. Typewriter progress counts visible Unicode characters, excluding tags. Invalid, malformed, mismatched, or unclosed tags produce project diagnostics with source file and line; literal braces are written `{{` and `}}`.

`nvl on` switches dialogue to full-screen narration; `nvl off` returns to the textbox. NVL lines accumulate until `nvl clear`. A character may opt into NVL with its `nvl` flag. The visible page is derived from VM state, so rollback and saves restore its mode and contents; backlog records every line once.

The theme's `[fonts] fallback = [...]` lists font files relative to `assets/` after the dialogue and interface fonts. Native players also try installed PingFang, STHeiti, Noto CJK, and Microsoft YaHei fonts. Web builds cannot inspect host fonts; web projects need to provide a fallback font in this list. Text layout permits line breaks between adjacent CJK characters.

## Staging

`stage <tag>` sets optional `x`, `y`, `scale`, `rotation`, and `alpha` properties. Add `over=<seconds>` and `at`, `linear`, or `ease` for a timeline; `at` snaps at its end, `linear` interpolates evenly, and `ease` uses smoothstep. `camera` accepts `x`, `y`, and `zoom` with the same timeline options. `shake <seconds> <amount>` and `flash "#rrggbb" <seconds>` create timed screen effects. Animation and effect state lives in the VM stage snapshot, so rollback and saves restore it; skip completes active timelines and effects immediately. Saves upgrade from version 3 to version 4.

Per-image shaders are deferred. The current renderer has one shared WebGPU/WGPU image pipeline, and a per-image hook would require an unstable shader ABI and a separate material/binding lifecycle; no shader command or placeholder API is introduced.

## Decisions

- Tags are parsed from localized text at render time, so translators can reorder markup while stable statement IDs remain tied to markup-normalized script source. Literal braces are unescaped for display and ID derivation.
- Ruby readings render smaller than their base text and count as visible typewriter characters; annotation positioning remains deferred.
- NVL page state is VM state, not a separate player presentation cache.
- Native players use installed system CJK fonts when available; web projects provide fallback fonts through the theme because browsers cannot read local system font files.
- Skip snaps active staging to its target instead of simulating animation time.
- Shaders remain deferred until the renderer has a stable material interface shared by both targets.

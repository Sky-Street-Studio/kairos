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

The language, compiler and interpreter work headless. The graphical player, the Electron editor, the AI writing assistant and Ren'Py import are not built yet.

| Crate | Role |
| --- | --- |
| `kairos-script` | Parser for `.kai` files, with diagnostics that keep parsing past errors |
| `kairos-vm` | Compiler to a flat instruction list, interpreter, rollback, saves |
| `kairos-cli` | `kairos check` and `kairos run --headless` |

Saves point at statements by a stable ID (label, source-text hash and occurrence), so a save keeps working after edits that leave the saved line and its call sites alone.

## Try it

```sh
cargo run -p kairos-cli -- check examples/hello-kairos
cargo run -p kairos-cli -- run --headless examples/hello-kairos --inputs examples/hello-kairos/inputs.txt
cargo test --workspace
```

## Project layout

```
kairos.toml        name, start label
script/*.kai       story scripts, compiled in file-name order
```

## License

MIT

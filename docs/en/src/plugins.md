# Plugins (WASM)

A plugin is a small WebAssembly module that publishes **one line of text** in the
island (and can ask for a bit of attention). It can be written in any language
that targets WASM (Rust, Zig, AssemblyScript, C…), and runs in a **sandbox**: no
access to files, network or system, only the few functions below.

Plugins are **off by default**: third-party code only runs if you ask for it
(Settings › General › "Run WASM plugins", or `[modules.plugins] enabled = true`).

## Installing a plugin

In `%APPDATA%\BoringWindows\plugins\` (**Plugins folder** button in the
settings), one folder per plugin:

```
plugins/
└─ my-plugin/
   ├─ plugin.toml
   └─ plugin.wasm
```

```toml
# plugin.toml
name = "My plugin"         # displayed name (default: folder name)
wasm = "plugin.wasm"       # WASM file in the folder (default)
interval_secs = 30         # delay between two calls, 5 to 3600 (default 30)
```

To run only some of them: `[modules.plugins] only = ["my-plugin"]`. A complete
example ships in
[`examples/plugins/hello`](https://github.com/yazouv/BoringWindows/tree/main/examples/plugins/hello).

## Writing a plugin

The module **exports**:

| Export | Purpose |
|---|---|
| `memory` | its linear memory (required: the host reads texts from it) |
| `bw_update()` | called at start, then every `interval_secs`; no parameter, no result |

and may **import** (module `"bw"`):

| Import | Purpose |
|---|---|
| `set_text(ptr: i32, len: i32)` | text shown (UTF-8, at most 120 characters) |
| `set_attention(level: i32)` | 0 nothing, 1 discreet (pill), 2 worth a look; "urgent" is reserved for the app |
| `log(ptr: i32, len: i32)` | message in BoringWindows' log |
| `now_unix() -> i64` | current time, seconds since 1970 |

Any other import (WASI, file system, network…) makes the plugin fail to load.

Minimal example in WAT:

```wat
(module
  (import "bw" "set_text" (func $set_text (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 16) "Hello")
  (func (export "bw_update")
    (call $set_text (i32.const 16) (i32.const 5))))
```

## Sandbox limits

- **Compute**: a budget per `bw_update` call; an endless loop is interrupted
  (error logged).
- **Memory**: 16 MB at most.
- **Errors**: a failing call shows "⚠ erreur"; after 3 failures in a row the
  plugin is stopped until the modules are reloaded.
- A plugin sees nothing of what the island does (music, calendar, Claude…): it
  can only publish text.

The island shows at most **two** plugin lines; `layout.view` can reuse them
through `plugin-rows` (see [Custom views](layouts.md)).

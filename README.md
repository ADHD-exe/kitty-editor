# Kitty Config Editor

`kitty-config-editor` is a keyboard-first TUI for browsing and editing Kitty terminal configuration.
It loads your current `kitty.conf`, resolves `include` files, uses bundled Kitty reference metadata for descriptions/defaults/validation, and provides dedicated screens for settings, themes, diffs, and keyboard shortcuts.

It runs in a normal terminal, works best inside Kitty, and can also be launched through the included custom-kitten wrapper in [`packaging/kitty_config_editor.py`](packaging/kitty_config_editor.py).

## Features

- Browse Kitty settings by category with inline descriptions, defaults, examples, and validation
- Fuzzy-search settings with `/`
- Parse `include` files recursively and show effective values from the full config tree
- Theme browser with directory scanning, filtering, preview/apply, and live color editing
- Save a theme as a new preset, then auto-save later live color edits back to that saved preset
- Shortcut browser with `custom`, `effective`, and `defaults` views
- Filter shortcuts, reassign shortcuts, add explicit map entries, and unmap defaults
- Built-in diff view before writing changes
- Full save and minimal save modes
- Optional post-save reload command
- Startup prompt that can open an existing config or create a starter config from defaults

## Requirements

- Rust toolchain
- Kitty terminal for the best experience
- A Kitty config, usually `~/.config/kitty/kitty.conf`
- Optional: a themes directory such as `~/.config/kitty/themes`

Notes:

- The TUI also works in a non-Kitty terminal.
- Theme preview and config reload use `kitty @ ...` by default, so those parts work best when Kitty remote control is available.
- The project has been tested primarily on Arch Linux / CachyOS.

## Install

There is no packaged installer in the repo yet, so the normal path is to build from source.

```bash
git clone <repo-url>
cd <repo-dir>
cargo build --release
```

The binary will be at:

```bash
target/release/kitty-config-editor
```

If you want it on your `PATH`, either install it with Cargo:

```bash
cargo install --path .
```

or copy the built binary manually:

```bash
install -Dm755 target/release/kitty-config-editor ~/.local/bin/kitty-config-editor
```

## Run

### Interactive startup prompt

Run without paths to open the startup screen:

```bash
cargo run --release
```

or, after installing the binary:

```bash
kitty-config-editor
```

If you omit one or both of `--current` and `--themes-dir`, the TUI opens a startup prompt that lets you:

- choose an existing `kitty.conf` or its containing directory
- choose a themes directory
- enable or disable backup creation
- create a new `kitty.conf` from defaults

When `Create new config from defaults` is enabled, the app creates a starter `kitty.conf` in the directory you choose.

### Run with explicit paths

```bash
kitty-config-editor \
  --current ~/.config/kitty/kitty.conf \
  --themes-dir ~/.config/kitty/themes
```

Force Kitty runtime features:

```bash
kitty-config-editor \
  --runtime kitty \
  --current ~/.config/kitty/kitty.conf \
  --themes-dir ~/.config/kitty/themes
```

Write full and minimal output to explicit locations:

```bash
kitty-config-editor \
  --current ~/.config/kitty/kitty.conf \
  --themes-dir ~/.config/kitty/themes \
  --out-full ~/.config/kitty/kitty.conf \
  --out-minimal ~/.config/kitty/kitty.min.conf
```

### CLI flags

- `--current`: path to `kitty.conf` or its containing directory
- `--themes-dir`: directory to scan for themes
- `--out-full`: destination for full-format output
- `--out-minimal`: destination for minimal override output
- `--enable-reload`: run the reload command after saves even when writing elsewhere
- `--reload-command`: override the default reload command (`kitty @ load-config ...`)
- `--runtime auto|plain|kitty|kitten`: choose terminal/runtime behavior
- `--tick-rate-ms <n>`: UI refresh/event polling interval
- `--no-kitty-keyboard`: disable Kitty keyboard protocol negotiation
- `--no-bracketed-paste`: disable bracketed paste support

## Using It Inside Kitty

### Overlay launch

For a direct Kitty binding without the wrapper:

```conf
map ctrl+alt+shift+k launch --type=overlay-main kitty-config-editor --runtime=kitty
```

### Custom kitten wrapper

The repo includes:

- [`packaging/kitty_config_editor.py`](packaging/kitty_config_editor.py): thin wrapper that launches the Rust binary with `--runtime=kitten`
- [`packaging/kitty.conf.example`](packaging/kitty.conf.example): example Kitty mappings

If you already use custom kittens, copy `packaging/kitty_config_editor.py` into that setup and map it like this:

```conf
map ctrl+alt+k kitten kitty_config_editor.py
```

The wrapper looks for `kitty-config-editor` in:

- `KITTY_CONFIG_EDITOR_BIN`
- `~/.local/bin/kitty-config-editor`
- `~/.cargo/bin/kitty-config-editor`
- `target/release/kitty-config-editor`
- `target/debug/kitty-config-editor`
- your normal `PATH`

## TUI Overview

### Settings browser

This is the main screen for editing normal Kitty options.

- `Up` / `Down`: move through settings
- `Left` / `Right` / `Tab`: move between panes
- `Enter` or `e`: edit the selected value
- `/`: search settings
- `r`: reset the selected setting to its default
- `c`: clear the current override
- `d`: open diff view

### Theme browser

Press `t` to open the theme selector.

- `Up` / `Down`: move through themes and preview them
- `/`: filter themes
- `Enter` or `Space`: apply the selected theme
- `e`: enter live theme editing, focused on the primary theme colors first
- `w` or `n`: save the current theme as a new preset
- `Esc`: leave theme mode

After you save a theme as a new preset, later live color edits are auto-saved back to that saved preset. Built-in/default themes are not auto-overwritten.

### Shortcut browser

Press `k` to open the shortcut browser.

- `1`: `custom` view
- `2`: `effective` view
- `3`: `defaults` view
- `/`: filter shortcuts
- `Up` / `Down` / `j` / `k`: move through rows
- `PgUp` / `PgDn` / `Home` / `End`: faster navigation
- `Enter` or `e`: reassign the selected shortcut
- `a`: add a new explicit `map` entry
- `x`: delete an explicit override or unmap a default shortcut

Editing a default shortcut turns it into an explicit override in the config.

### Save, diff, help, and quit

- `s`: save full config
- `S`: save minimal overrides
- `R`: save full config and force reload
- `?`: help popup
- `q`: quit
- `Esc`: cancel/close the current popup or return to the main browser

## Save Behavior

- Full save rewrites the main config in stable category order while preserving the leading non-setting block.
- Minimal save writes only changed settings, theme include changes, and explicit shortcut overrides/unmaps.
- When saving to the live main config, the app attempts a reload with `kitty @ load-config` unless you override it with `--reload-command`.
- If backup creation is enabled in the startup prompt, full saves to the main config create a sibling backup directory first.
- Included files are read for effective values, but the editor writes the main config; when using the editable theme wrapper flow it also writes the theme artifact file.

## Current Limitations

- Shortcut editing currently focuses on reassigning shortcuts, not arbitrary action editing for every `map` form.
- Removed shortcut rows still need manual repair in `kitty.conf`.
- Included files other than the theme artifact are not rewritten individually.
- The bundled reference metadata tracks the Kitty snapshot shipped with this repo; new upstream options require a repo update.

## Development

Run the test suite with:

```bash
cargo test
```

The package name is `kitty-config-editor`, version `0.2.0`, and the crate is licensed under MIT.
# kitty-editor
# kitty-editor
# kitty-editor
# kitty-editor
# kitty-editor
# kitty-editor

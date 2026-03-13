# Kitty Config Editor

TUI for editing Kitty config files.

## Dependencies

- `git`
- Rust toolchain
- Kitty terminal if you want live preview and reload support

## Install

Clone the repo and build it:

```bash
git clone <repo-url>
cd kitty-editor
cargo build --release
```

Binary path:

```bash
target/release/kitty-config-editor
```

Optional install to your `PATH`:

```bash
cargo install --path .
```

## Run

Open the TUI with the startup prompt:

```bash
cargo run --release
```

Or run the built binary:

```bash
kitty-config-editor
```

Run with explicit paths:

```bash
kitty-config-editor \
  --current ~/.config/kitty/kitty.conf \
  --themes-dir ~/.config/kitty/themes
```

## Flags

- `--current <path>`: path to `kitty.conf` or its directory
- `--themes-dir <path>`: themes directory
- `--out-full <path>`: full output path
- `--out-minimal <path>`: minimal output path
- `--enable-reload`: reload after saves even when writing somewhere else
- `--reload-command <cmd>`: custom reload command
- `--runtime <auto|plain|kitty|kitten>`: runtime mode
- `--tick-rate-ms <n>`: UI poll interval
- `--no-kitty-keyboard`: disable Kitty keyboard protocol
- `--no-bracketed-paste`: disable bracketed paste

Full CLI help:

```bash
kitty-config-editor --help
```

## Test

```bash
cargo test
```

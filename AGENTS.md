# ainput

Rust library + binary for multiplexing physical touchscreen input with virtual touch injection via `uinput`.

## Build & Run

```bash
cargo build                        # native Linux build
cargo run --example android_tui    # run TUI locally (needs root + touchscreen)
cargo ndk build --example android_tui --release -t arm64-v8a  # Android cross-compile
```

**Prerequisites:** Rust edition 2024 (1.85+), root access for device grab and `/dev/uinput`. For Android: `cargo-ndk`, NDK 27, `adb` with rooted device.

## Deploy to Android

```bash
adb push target/aarch64-linux-android/release/examples/android_tui /data/local/tmp
adb shell -t 'su -c "/data/local/tmp/android_tui"'   # -t is required for TUI
```

## Architecture

- `src/device.rs` — scans `/dev/input/event*`, scores and selects the best direct MT-B touchscreen
- `src/multiplexer.rs` — reads physical events, maintains slot state, emits complete MT-B frames
- `src/uinput.rs` — creates/manages the virtual uinput touchscreen with matching capabilities
- `src/lib.rs` — public API: `TouchDevice`, `TouchMultiplexer`, `Point`, `DisplaySize`
- `src/main.rs` — TUI binary using ratatui

## Environment

`AINPUT_TOUCH_DEVICE` — override autodetection with an explicit `/dev/input/eventX` path.

## Conventions

- `deploy-n-run.sh` is a local convenience script (gitignored). Do not commit it.
- No test suite currently exists.
- Edition 2024 features are in use.

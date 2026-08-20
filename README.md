# ainput

`ainput` captures a physical touchscreen via `EVIOCGRAB`, creates a virtual `uinput` device, and multiplexes both physical and virtual contacts into a single stream. This allows injecting touch events while preserving the original touch input.

## Features

- **Autodetection**: automatically finds the best direct MT-B touchscreen via evdev
- **Exclusive grab**: captures the physical device so the original handler is bypassed
- **Virtual touch injection**:  add a virtual contact alongside physical ones
- **Stateful MT-B protocol**: correctly reconstructs slot state per frame
- **TUI**: built-in terminal UI for testing and debugging

## Prerequisites

- **Root access**: `ainput` needs `EVIOCGRAB` on the physical device and `/dev/uinput` to create the virtual one
- **cargo-ndk**: for cross-compiling to Android (`cargo install cargo-ndk`)
- **NDK**: install via `sdkmanager "ndk;27.0.12077973"` or Android Studio
- **adb**: with a connected device that has root (`su`)

## Install 
```
cargo add --git https://github.com/0x41337/ainput
```

## Cross-compiling for Android

Add the Android target once:

```
rustup target add aarch64-linux-android
```

Build with cargo-ndk:

```
cargo ndk build --example android_tui --release -t arm64-v8a
```

## Deploying and running

Push the binary and run it with root:

```
adb push target/aarch64-linux-android/release/examples/android_tui /data/local/tmp
adb shell -t 'su -c "/data/local/tmp/android_tui"'
```

> **Important:** the `-t` flag allocates a pseudo-TTY. Without it the TUI will not render.

## Usage as a library

```rust
use ainput::{Point, TouchDevice, TouchMultiplexer};

fn main() -> std::io::Result<()> {
    let touchscreen = TouchDevice::detect()?;
    let mut mux = TouchMultiplexer::open(touchscreen)?;

    // Move virtual touch
    mux.touch_move(Point::new(360, 800))?;

    // Tap
    mux.touch_down(Point::new(360, 800))?;
    mux.touch_up()?;

    Ok(())
}
```

## Examples

| Example | Description |
|---------|-------------|
| `android_tui` | Interactive TUI for testing touch injection |
| `android_press_center` | Presses the center of the screen for 3 seconds |

Run locally (if you have a Linux touchscreen):

```
cargo run --example android_tui --release
```

Or cross-compile and deploy to Android as described above.

## TUI controls

| Key | Action |
|-----|--------|
| Arrow keys | Move virtual touch |
| Enter | Virtual touch DOWN |
| Space | Virtual touch UP |
| q | Exit |

## Environment

| Variable | Description |
|----------|-------------|
| `AINPUT_TOUCH_DEVICE` | Override autodetection with an explicit `/dev/input/eventX` path |

## Architecture

```
Physical touchscreen ──> EVIOCGRAB ──> ainput ──> Virtual uinput device
                                                         │
                                                    Virtual touch
                                                    (keyboard input)
```

1. **Detection** (`device.rs`) — scans `/dev/input/event*` for direct MT-B touchscreens, scores candidates, and selects the best one
2. **Multiplexer** (`multiplexer.rs`) — reads physical events, maintains slot state, and emits complete MT-B frames to the virtual device
3. **Virtual device** (`uinput.rs`) — creates and manages the `uinput` touchscreen with matching capabilities

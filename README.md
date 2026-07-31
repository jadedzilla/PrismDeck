# PrismDeck Launcher

PrismDeck is a controller-first launcher for Prism Launcher modpacks, built in Rust as a single native binary.

It is designed with Steam Big Picture-style navigation, dynamic controller hints, and a polished launcher interface for quickly selecting and launching Prism Launcher modpack instances from both native and Flatpak installations.

## Features

- Detects Prism Launcher modpack instances from native and Flatpak Prism Launcher installations
- Controller-first navigation with D-pad and left stick support
- Dynamic button prompts based on detected controller type (Xbox, PlayStation, generic)
- Single binary desktop launcher built with `eframe` and `gilrs`

## Build

Make sure you have a Rust toolchain installed and available on your `PATH`.

```bash
cargo build --release
```

The built binary will be available under `target/release/prismdeck-launcher`.

## Run

```bash
cargo run --release
```

Or execute the binary directly:

```bash
./target/release/prismdeck-launcher
```

## Controls

- Navigate: D-pad left/right or left stick left/right on controller
- Select: Confirm button (`A`, `Cross`, or `South`) or `Enter`
- Cancel / Back: Cancel popup, go back, or press `Esc`
- Refresh: `Y`, `Triangle`, or `North` on controller, or `R` on keyboard

## Notes

- Use the refresh control to rediscover Prism Launcher modpacks after installation or configuration changes.
- The launch popup gives a 3-second countdown before launching, allowing cancellation with the same confirm or back button.

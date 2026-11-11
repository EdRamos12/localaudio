# LocalAudio

Stream what this computer is playing to an Android phone on the same network. There are no ads and no account.

The desktop app is a native Rust program. On Linux it records the PipeWire monitor of the selected output. On Windows it records the default speakers with WASAPI loopback. The phone app is Kotlin and decodes Opus with libopus.

## Desktop

From `desktop/`:

```bash
cargo run --release
```

Headless, with a 440 Hz test tone instead of the speakers:

```bash
cargo run --release -- --headless --source tone
```

Capture the current default output:

```bash
cargo run --release -- --headless
```

The PC listens on TCP `47321` and UDP `47322`. Phones on the same LAN discover it, or you can type the PC address in the phone app. "Play only on the phone" creates a PipeWire sink named `LocalAudio-Speakers` and makes it the default output, so the PC itself stays quiet.

Windows builds are cross-compiled from Linux:

```bash
rustup target add x86_64-pc-windows-gnu
# Arch: pacman -S mingw-w64-gcc
cargo build --release --target x86_64-pc-windows-gnu
```

That Windows target was not run on this machine.

## Android

The phone and the PC have to be on the same LAN. Install `android/app/build/outputs/apk/debug/app-debug.apk`, open LocalAudio, and tap the PC.

`ANDROID_HOME` should point at an SDK with platform 35, build-tools 35.0.0, CMake 3.22.1, and NDK 27.2.12479018. This debug build includes `arm64-v8a` only. Opus 1.5.2 is vendored under `android/app/src/main/cpp/opus` (BSD license, see that directory's `COPYING`).

## Protocol

See [protocol/SPEC.md](protocol/SPEC.md).

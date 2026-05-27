# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-05-27

Initial release.

### Hardware

- Targets the [Kiwi Electronics / Cyntech English Wordclock kit][kit]
  (8×8 = 64 WS2812 LEDs on Raspberry Pi GPIO 18 / PWM0).
- Tested on Raspberry Pi 3 (32-bit Raspberry Pi OS Legacy / Bullseye).

### Features

- Drives the panel via [`rs_ws281x`][rs] (no external `pigpiod` / NeoPixel
  Python dependency).
- LED index tables ported verbatim from the official Cyntech Python reference,
  so the word layout matches the panel shipped with the kit.
- Each letter is assigned an independent random vivid color (uniform random
  hue, full HSV saturation/value) every time the displayed time changes;
  colors stay stable between updates so the panel does not flicker.
- Rounds hours up after `:32` and falls through to `TWELVE` for hour 0 / 12,
  matching the Python reference.

### Cross-compilation

- Builds on macOS with `cargo zigbuild --target
  armv7-unknown-linux-musleabihf --release` against musl libc; the resulting
  binary is statically linked (~420 KB) and runs on any 32-bit Raspberry Pi OS
  without runtime library dependencies.
- `Cargo.toml`'s `[build-dependencies]` forces `bindgen` / `clang-sys` into
  runtime (`dlopen`) mode, so `libclang.dylib` is loaded via `LIBCLANG_PATH`
  alone. This avoids the `dyld: Library not loaded: @rpath/libclang.dylib`
  error that occurs when bindgen links libclang dynamically (cargo silently
  rewrites `DYLD_FALLBACK_LIBRARY_PATH` for build scripts, making `[env]` in
  `.cargo/config.toml` unable to fix it).
- `.cargo/config.toml` sets `LIBCLANG_PATH=/opt/homebrew/opt/llvm/lib` so a
  bare `cargo zigbuild` "just works" on Apple-Silicon Macs with Homebrew LLVM.

### Deployment

- `deploy.sh` builds, strips, and `scp`s the binary to `pi@wordclock.local`
  using SSH connection multiplexing (single password prompt across multiple
  `scp` / `ssh` calls) and a short `/tmp/wc-ssh.<pid>/s` control-socket path
  that fits within macOS's 104-byte `sockaddr_un` limit.
- `deploy.sh --install` additionally installs the binary to `/usr/local/bin`,
  installs `wordclock.service`, and enables/starts the unit. `ssh -t` is used
  for the install step so `sudo` can prompt for a password interactively.
- Ships a `wordclock.service` systemd unit that runs as `root` (needed for
  `/dev/mem` access) and restarts on failure.

[kit]: https://www.kiwi-electronics.com/nl/wordclock-kit-voor-raspberry-pi-engels-10360
[rs]: https://github.com/rpi-ws281x/rpi-ws281x-rust

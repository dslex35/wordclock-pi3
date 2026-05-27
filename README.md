# wordclock-pi3

Rust driver for the [Kiwi Electronics / Cyntech Wordclock Kit][kit]
(8×8 = 64 WS2812 LEDs on a Raspberry Pi GPIO 18 PWM0).

Cross-compiled on macOS with `cargo-zigbuild` and deployed to a Raspberry Pi 3
running 32-bit Raspberry Pi OS (Legacy / Bullseye).

[kit]: https://www.kiwi-electronics.com/nl/wordclock-kit-voor-raspberry-pi-engels-10360

## Hardware

- Raspberry Pi 3 (or B+, 2, Zero), 32-bit Raspberry Pi OS.
- Cyntech 8×8 WS2812 panel, wired to:
  - `+5V`  → Pi 5V (pin 2 or 4)
  - `GND`  → Pi GND (pin 6)
  - `DIN`  → Pi GPIO 18 (pin 12, PWM0)

## One-time Pi setup

WS2812 timing on the Pi uses the PWM hardware. On the Pi 3 that conflicts with
the onboard audio, so audio must be disabled.

```bash
ssh pi@wordclock.local

# Disable onboard audio so PWM0 is free.
sudo sed -i 's/^dtparam=audio=on/dtparam=audio=off/' /boot/config.txt \
  || echo 'dtparam=audio=off' | sudo tee -a /boot/config.txt
# (On newer Pi OS the file lives at /boot/firmware/config.txt - adjust accordingly.)

sudo reboot
```

## One-time macOS setup

```bash
brew install zig llvm
cargo install cargo-zigbuild
rustup target add armv7-unknown-linux-musleabihf
```

- `cargo-zigbuild` uses `zig cc` as the C compiler/linker, so the bundled C
  library inside `rs_ws281x` cross-compiles cleanly without an extra GCC toolchain.
- `brew install llvm` is needed because `rs_ws281x`'s `build.rs` runs `bindgen`,
  which requires `libclang.dylib` on the host. Apple's Command Line Tools
  clang doesn't expose `libclang.dylib` to dyld, so Homebrew's LLVM is used
  instead.
- The resulting binary is statically linked against musl libc, so it runs on
  any 32-bit Raspberry Pi OS without dependency hassles.

## Build

The explicit command you asked for:

```bash
cargo zigbuild --target armv7-unknown-linux-musleabihf --release
```

`.cargo/config.toml` sets `LIBCLANG_PATH=/opt/homebrew/opt/llvm/lib`, and
`Cargo.toml` forces `bindgen`/`clang-sys` into runtime/dlopen mode so dyld
never needs to resolve `@rpath/libclang.dylib`. If your Homebrew prefix is
`/usr/local` (Intel Macs), override with `LIBCLANG_PATH=/usr/local/opt/llvm/lib
cargo zigbuild ...` or edit the path in `.cargo/config.toml`.

The output binary is at:

```
target/armv7-unknown-linux-musleabihf/release/wordclock-rust-pwm
```

There is also a `cargo pi-build` alias and a `deploy.sh` helper.

## Deploy

```bash
# Build + scp the binary to pi@wordclock.local
./deploy.sh

# Build + install as a systemd service that starts on boot
./deploy.sh --install

# Target a different host (override the default `pi@wordclock.local`):
PI_HOST=pi@your-host.local ./deploy.sh
```

After `--install`, useful commands:

```bash
ssh pi@wordclock.local 'sudo systemctl status wordclock'
ssh pi@wordclock.local 'journalctl -fu wordclock'
ssh pi@wordclock.local 'sudo systemctl restart wordclock'
```

## Run manually (without systemd)

The `rs_ws281x` library writes to `/dev/mem`, so it must be run as root:

```bash
ssh pi@wordclock.local
sudo ~/wordclock-rust-pwm
```

## Notes

- LED indices in `src/main.rs` are taken directly from the official Cyntech
  Python reference ([`CyntechUK/Wordclock`](https://github.com/CyntechUK/Wordclock)),
  so the word layout matches the panel shipped with the Kiwi Electronics kit.
- DMA channel is `10`. Some older guides say `5`; do **not** use 5 on modern
  Raspberry Pi OS – it conflicts with the SD card controller.
- Brightness is hard-coded to `120/255`. Tweak `brightness(..)` in `main.rs`
  if you want it brighter / dimmer.
- Color is a warm amber (`[B, G, R, W] = [30, 140, 255, 0]`). Edit the `on`
  array in `render()` to change it.

## Troubleshooting

- **"Failed to create controller - run with sudo!"** – run with `sudo`, or use
  the systemd service which runs as root.
- **All LEDs flicker / show garbage** – verify audio is disabled
  (`dtparam=audio=off`) and the Pi was rebooted afterwards.
- **No LEDs light up** – check the data line really is on GPIO 18 (pin 12),
  and that the panel is being powered with a steady 5 V.
- **`Library not loaded: @rpath/libclang.dylib` during build** – you removed
  the `[build-dependencies]` block from `Cargo.toml` (which forces bindgen
  into runtime/dlopen mode), or `LIBCLANG_PATH` doesn't point at a directory
  containing `libclang.dylib`. Run `brew install llvm` and check the path in
  `.cargo/config.toml`.

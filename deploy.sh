#!/usr/bin/env bash
# Build for armv7 musl with cargo-zigbuild and deploy to the Raspberry Pi.
#
# Requirements on macOS:
#   brew install zig llvm
#   cargo install cargo-zigbuild
#   rustup target add armv7-unknown-linux-musleabihf
#
# Usage:
#   ./deploy.sh                       # builds + scp to pi@wordclock.local
#   PI_HOST=pi@your-host.local ./deploy.sh   # override the target host
#   ./deploy.sh --install              # also install systemd unit + enable service
#
# Tip: set up an SSH key so you're not prompted for a password each step:
#   ssh-copy-id pi@wordclock.local

set -euo pipefail

PI_HOST="${PI_HOST:-pi@wordclock.local}"
TARGET="armv7-unknown-linux-musleabihf"
BIN_NAME="wordclock-rust-pwm"

INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --install) INSTALL=1 ;;
    *) echo "Unknown arg: $arg" >&2; exit 1 ;;
  esac
done

# .cargo/config.toml sets LIBCLANG_PATH and bindgen uses runtime/dlopen mode,
# so a bare `cargo zigbuild` works. We still allow overriding LIBCLANG_PATH
# via the environment for non-Apple-Silicon / non-Homebrew setups.
if [[ -z "${LIBCLANG_PATH:-}" && ! -f /opt/homebrew/opt/llvm/lib/libclang.dylib ]]; then
  if [[ -f /usr/local/opt/llvm/lib/libclang.dylib ]]; then
    export LIBCLANG_PATH=/usr/local/opt/llvm/lib
  else
    echo "WARN: libclang.dylib not found in /opt/homebrew or /usr/local. Install with: brew install llvm" >&2
  fi
fi

echo ">> Building $BIN_NAME for $TARGET (release)"
cargo zigbuild --target "$TARGET" --release

BIN_PATH="target/${TARGET}/release/${BIN_NAME}"
if [[ ! -f "$BIN_PATH" ]]; then
  echo "Build did not produce $BIN_PATH" >&2
  exit 1
fi

echo ">> Stripping binary"
if command -v llvm-strip >/dev/null 2>&1; then
  llvm-strip "$BIN_PATH" || true
fi

# Reuse a single SSH connection for all scp/ssh calls below, so the user is
# prompted for the SSH password at most once per `deploy.sh` invocation.
# macOS limits Unix-socket paths to 104 bytes, so keep this short (don't use
# $TMPDIR which is under /var/folders/... and easily exceeds the limit).
SSH_CTRL_DIR="/tmp/wc-ssh.$$"
mkdir -p "$SSH_CTRL_DIR"
chmod 700 "$SSH_CTRL_DIR"
SSH_CTRL="${SSH_CTRL_DIR}/s"
SSH_OPTS=(-o "ControlMaster=auto" -o "ControlPath=${SSH_CTRL}" -o "ControlPersist=60")
cleanup() {
  ssh "${SSH_OPTS[@]}" -O exit "$PI_HOST" >/dev/null 2>&1 || true
  rm -rf "$SSH_CTRL_DIR"
}
trap cleanup EXIT

echo ">> Opening SSH connection to $PI_HOST (you may be prompted once)"
ssh "${SSH_OPTS[@]}" -fN "$PI_HOST"

echo ">> Copying binary to $PI_HOST"
scp "${SSH_OPTS[@]}" "$BIN_PATH" "$PI_HOST:~/${BIN_NAME}"

if [[ "$INSTALL" -eq 1 ]]; then
  echo ">> Installing systemd unit on $PI_HOST"
  scp "${SSH_OPTS[@]}" wordclock.service "$PI_HOST:/tmp/wordclock.service"
  # -t allocates a tty so `sudo` can prompt for the remote user's password
  # interactively if passwordless sudo isn't configured.
  ssh "${SSH_OPTS[@]}" -t "$PI_HOST" "
    set -e
    sudo install -m 755 \$HOME/${BIN_NAME} /usr/local/bin/${BIN_NAME}
    sudo install -m 644 /tmp/wordclock.service /etc/systemd/system/wordclock.service
    sudo systemctl daemon-reload
    sudo systemctl enable --now wordclock.service
    sudo systemctl status wordclock.service --no-pager
  "
  echo ">> Service installed. Tail logs with: ssh $PI_HOST 'journalctl -fu wordclock'"
else
  echo ">> Done. Run it on the Pi with:  ssh -t $PI_HOST 'sudo ~/${BIN_NAME}'"
fi

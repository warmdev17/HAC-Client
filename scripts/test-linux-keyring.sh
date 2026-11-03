#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
hac_test_dir=$(mktemp -d /tmp/hac-keyring.XXXXXX)
export XDG_DATA_HOME="$hac_test_dir/data"
export XDG_CONFIG_HOME="$hac_test_dir/config"
export XDG_RUNTIME_DIR="$hac_test_dir/runtime"
mkdir -m 700 -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_RUNTIME_DIR"
unset GNOME_KEYRING_CONTROL GNOME_KEYRING_PID SSH_AUTH_SOCK
export HAC_TEST_PRIVATE_BUS=1
export HAC_BASE_URL=https://provider.example/v1
exec dbus-run-session -- bash -c '
  set -euo pipefail
  printf "hac-test-only-password" | gnome-keyring-daemon --foreground --unlock --components=secrets --control-directory="$XDG_RUNTIME_DIR/keyring" >/dev/null 2>&1 &
  hac_daemon=$!
  trap "kill $hac_daemon 2>/dev/null || true" EXIT
  for i in {1..50}; do
    if busctl --address="$DBUS_SESSION_BUS_ADDRESS" --list | grep org.freedesktop.secrets >/dev/null; then break; fi
    sleep 0.1
  done
  busctl --address="$DBUS_SESSION_BUS_ADDRESS" --list | grep org.freedesktop.secrets >/dev/null
  cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked --test linux_secret_service --no-run
  timeout 30s cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked --test linux_secret_service -- --ignored
'

#!/usr/bin/env bash

set -euo pipefail

for required_command in pgrep xvfb-run xauth xprop; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    echo "Missing X11 smoke dependency: $required_command" >&2
    exit 1
  fi
done

binary="${1:-target/release/dwm_status}"

if [[ ! -x "$binary" ]]; then
  echo "Missing release binary: $binary" >&2
  exit 1
fi

smoke_dir=$(mktemp -d)
trap 'rm -rf -- "$smoke_dir"' EXIT

config_path="$smoke_dir/config.toml"
stderr_path="$smoke_dir/dwm_status.stderr"

cat >"$config_path" <<'EOF'
features = ["clock"]

[clock]
format = "ci-x11-ok-%S"
timezone = "UTC"
EOF

xvfb-run --auto-servernum --server-args="-screen 0 640x480x24" \
  bash -euo pipefail -c '
    binary=$1
    config_path=$2
    stderr_path=$3
    marker=ci-x11-ok-

    "$binary" --config "$config_path" 2>"$stderr_path" &
    status_pid=$!
    trap '\''kill "$status_pid" 2>/dev/null || true; wait "$status_pid" 2>/dev/null || true'\'' EXIT

    for _attempt in $(seq 1 50); do
      if ! kill -0 "$status_pid" 2>/dev/null; then
        echo "dwm_status stopped before publishing X11 properties" >&2
        exit 1
      fi

      wm_name=$(xprop -root WM_NAME 2>/dev/null || true)
      net_wm_name=$(xprop -root _NET_WM_NAME 2>/dev/null || true)

      if [[ "$wm_name" == *"$marker"* && "$net_wm_name" == *"$marker"* ]]; then
        break
      fi

      sleep 0.1
    done

    if [[ "$wm_name" != *"$marker"* || "$net_wm_name" != *"$marker"* ]]; then
      echo "Timed out waiting for WM_NAME and _NET_WM_NAME" >&2
      xprop -root WM_NAME _NET_WM_NAME >&2 || true
      exit 1
    fi

    xvfb_pid=$(pgrep -o -P "$PPID" -x Xvfb || true)

    if [[ -z "$xvfb_pid" ]]; then
      echo "Failed to identify the Xvfb server process" >&2
      exit 1
    fi

    kill "$xvfb_pid"

    for _attempt in $(seq 1 50); do
      if ! kill -0 "$status_pid" 2>/dev/null; then
        set +e
        wait "$status_pid"
        status=$?
        set -e

        if [[ "$status" -eq 0 ]]; then
          echo "dwm_status exited successfully after losing X11" >&2
          exit 1
        fi

        if ! grep -Eq "Failed to (queue|write|flush).*X11" "$stderr_path"; then
          echo "dwm_status did not report the fatal X11 write" >&2
          cat "$stderr_path" >&2
          exit 1
        fi

        exit 0
      fi

      sleep 0.1
    done

    echo "dwm_status stayed alive after losing X11" >&2
    cat "$stderr_path" >&2
    exit 1
  ' bash "$binary" "$config_path" "$stderr_path"

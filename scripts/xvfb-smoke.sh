#!/usr/bin/env bash

set -euo pipefail

for required_command in Xvfb xprop; do
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
xvfb_pid=""
status_pid=""
trap 'if [[ -n "$status_pid" ]]; then kill "$status_pid" 2>/dev/null || true; wait "$status_pid" 2>/dev/null || true; fi; if [[ -n "$xvfb_pid" ]]; then kill "$xvfb_pid" 2>/dev/null || true; wait "$xvfb_pid" 2>/dev/null || true; fi; rm -rf -- "$smoke_dir"' EXIT

config_path="$smoke_dir/config.toml"
stderr_path="$smoke_dir/dwm_status.stderr"
xvfb_log_path="$smoke_dir/xvfb.log"
display_path="$smoke_dir/display"

cat >"$config_path" <<'EOF'
features = ["clock"]

[clock]
glyph = " "
format = "ci-x11-ok-%S"
timezone = "UTC"
EOF

exec 3>"$display_path"
Xvfb -displayfd 3 -screen 0 640x480x24 -nolisten tcp -ac >"$xvfb_log_path" 2>&1 &
xvfb_pid=$!
exec 3>&-

display=""

for _attempt in $(seq 1 50); do
  if ! kill -0 "$xvfb_pid" 2>/dev/null; then
    echo "Xvfb stopped before accepting connections" >&2
    cat "$xvfb_log_path" >&2
    exit 1
  fi

  if [[ -s "$display_path" ]]; then
    display=":$(<"$display_path")"

    if DISPLAY="$display" xprop -root >/dev/null 2>&1; then
      break
    fi
  fi

  sleep 0.1
done

if [[ -z "$display" ]] || ! DISPLAY="$display" xprop -root >/dev/null 2>&1; then
  echo "Timed out waiting for Xvfb" >&2
  cat "$xvfb_log_path" >&2
  exit 1
fi

DISPLAY="$display" "$binary" --config "$config_path" 2>"$stderr_path" &
status_pid=$!
wm_pattern='^WM_NAME = "▏ ci-x11-ok-[0-9]{2}▕"$'
net_wm_pattern='^_NET_WM_NAME = "▏ ci-x11-ok-[0-9]{2}▕"$'
wm_name=""
net_wm_name=""

for _attempt in $(seq 1 50); do
  if ! kill -0 "$status_pid" 2>/dev/null; then
    echo "dwm_status stopped before publishing X11 properties" >&2
    cat "$stderr_path" >&2
    exit 1
  fi

  wm_name=$(DISPLAY="$display" xprop -root -notype -f WM_NAME 8u WM_NAME 2>/dev/null || true)
  net_wm_name=$(DISPLAY="$display" xprop -root -notype -f _NET_WM_NAME 8u _NET_WM_NAME 2>/dev/null || true)
  wm_payload=${wm_name#WM_NAME = }
  net_wm_payload=${net_wm_name#_NET_WM_NAME = }

  if [[ "$wm_name" =~ $wm_pattern ]] &&
    [[ "$net_wm_name" =~ $net_wm_pattern ]] &&
    [[ "$wm_payload" == "$net_wm_payload" ]]; then
    break
  fi

  sleep 0.1
done


if [[ ! "$wm_name" =~ $wm_pattern ]] ||
  [[ ! "$net_wm_name" =~ $net_wm_pattern ]] ||
  [[ "$wm_payload" != "$net_wm_payload" ]]; then
  echo "Timed out waiting for exact matching WM_NAME and _NET_WM_NAME payloads" >&2
  DISPLAY="$display" xprop -root WM_NAME _NET_WM_NAME >&2 || true
  exit 1
fi

kill "$xvfb_pid"
wait "$xvfb_pid" 2>/dev/null || true
xvfb_pid=""

for _attempt in $(seq 1 50); do
  if ! kill -0 "$status_pid" 2>/dev/null; then
    set +e
    wait "$status_pid"
    status=$?
    set -e
    status_pid=""

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

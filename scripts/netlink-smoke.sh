#!/usr/bin/env bash

set -euo pipefail

for required_command in Xvfb ip sudo unshare xprop; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    echo "Missing netlink smoke dependency: $required_command" >&2
    exit 1
  fi
done

binary="${1:-target/release/dwm_status}"

if [[ ! -x "$binary" ]]; then
  echo "Missing release binary: $binary" >&2
  exit 1
fi

binary=$(realpath "$binary")
smoke_dir=$(mktemp -d)
xvfb_pid=""
trap 'if [[ -n "$xvfb_pid" ]]; then kill "$xvfb_pid" 2>/dev/null || true; wait "$xvfb_pid" 2>/dev/null || true; fi; rm -rf -- "$smoke_dir"' EXIT

config_path="$smoke_dir/config.toml"
stderr_path="$smoke_dir/dwm_status.stderr"
xvfb_log_path="$smoke_dir/xvfb.log"
display_path="$smoke_dir/display"
namespace_script="$smoke_dir/run-in-namespace.sh"

cat >"$config_path" <<'EOF'
features = ["connectivity"]

[connectivity]
idle = 60
EOF

cat >"$namespace_script" <<'EOF'
#!/usr/bin/env bash

set -euo pipefail

status_pid=""
trap 'if [[ -n "$status_pid" ]]; then kill "$status_pid" 2>/dev/null || true; wait "$status_pid" 2>/dev/null || true; fi' EXIT

"$BINARY" --config "$CONFIG_PATH" 2>"$STDERR_PATH" &
status_pid=$!
initial_pattern='^WM_NAME = "▏no-iface no-gw (no-dns|stub|dns|mixed)▕"$'
link_pattern='^WM_NAME = "▏E:ci-dummy no-gw (no-dns|stub|dns|mixed)▕"$'
route_pattern='^WM_NAME = "▏E:ci-dummy gw (no-dns|stub|dns|mixed)▕"$'
wm_name=""

for _attempt in $(seq 1 50); do
  if ! kill -0 "$status_pid" 2>/dev/null; then
    echo "dwm_status stopped before its initial connectivity sample" >&2
    cat "$STDERR_PATH" >&2
    exit 1
  fi

  wm_name=$(xprop -root -notype -f WM_NAME 8u WM_NAME 2>/dev/null || true)

  if [[ "$wm_name" =~ $initial_pattern ]]; then
    break
  fi

  sleep 0.1
done

if [[ ! "$wm_name" =~ $initial_pattern ]]; then
  echo "Timed out waiting for the initial connectivity sample" >&2
  xprop -root WM_NAME >&2 || true
  cat "$STDERR_PATH" >&2
  exit 1
fi

ip link add ci-dummy type dummy
ip link set ci-dummy up

for _attempt in $(seq 1 50); do
  wm_name=$(xprop -root -notype -f WM_NAME 8u WM_NAME 2>/dev/null || true)

  if [[ "$wm_name" =~ $link_pattern ]]; then
    break
  fi

  sleep 0.1
done

if [[ ! "$wm_name" =~ $link_pattern ]]; then
  echo "A link event did not wake connectivity rendering" >&2
  xprop -root WM_NAME >&2 || true
  cat "$STDERR_PATH" >&2
  exit 1
fi

ip route add default dev ci-dummy metric 42

for _attempt in $(seq 1 50); do
  wm_name=$(xprop -root -notype -f WM_NAME 8u WM_NAME 2>/dev/null || true)

  if [[ "$wm_name" =~ $route_pattern ]]; then
    break
  fi

  sleep 0.1
done


if [[ ! "$wm_name" =~ $route_pattern ]]; then
  echo "A route event did not wake connectivity rendering" >&2
  xprop -root WM_NAME >&2 || true
  cat "$STDERR_PATH" >&2
  exit 1
fi

if [[ -s "$STDERR_PATH" ]]; then
  echo "dwm_status reported an error during the netlink smoke test" >&2
  cat "$STDERR_PATH" >&2
  exit 1
fi
EOF

chmod 755 "$smoke_dir" "$namespace_script"
chmod 644 "$config_path"

exec 3>"$display_path"
Xvfb -displayfd 3 -screen 0 640x480x24 -nolisten tcp -ac >"$xvfb_log_path" 2>&1 &
xvfb_pid=$!
exec 3>&-

display=""

for _attempt in $(seq 1 50); do
  if ! kill -0 "$xvfb_pid" 2>/dev/null; then
    echo "Xvfb stopped before accepting netlink smoke connections" >&2
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
  echo "Timed out waiting for netlink smoke Xvfb" >&2
  cat "$xvfb_log_path" >&2
  exit 1
fi

sudo --non-interactive /usr/bin/env \
  DISPLAY="$display" \
  BINARY="$binary" \
  CONFIG_PATH="$config_path" \
  STDERR_PATH="$stderr_path" \
  /usr/bin/unshare --net -- "$namespace_script"

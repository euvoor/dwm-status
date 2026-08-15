#!/usr/bin/env bash

set -euo pipefail

for required_command in make stat; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    echo "Missing install smoke dependency: $required_command" >&2
    exit 1
  fi
done

smoke_dir=$(mktemp -d)
trap 'rm -rf -- "$smoke_dir"' EXIT

default_bin_dir="$smoke_dir/default/bin"
default_config_dir="$smoke_dir/default/config"
alternate_bin_dir="$smoke_dir/alternate/bin"
alternate_config_dir="$smoke_dir/alternate/config"

make --no-print-directory install \
  BINDIR="$default_bin_dir" \
  CONFIG_DIR="$default_config_dir"

if [[ ! -x "$default_bin_dir/dwm_status" ]] ||
  [[ "$(stat -c '%a' "$default_bin_dir/dwm_status")" != "755" ]]; then
  echo "Default install did not create a mode-755 dwm_status binary" >&2
  exit 1
fi

make --no-print-directory install \
  BINDIR="$alternate_bin_dir" \
  CONFIG_DIR="$alternate_config_dir" \
  BIN="dwm-status-ci"

if [[ ! -x "$alternate_bin_dir/dwm-status-ci" ]] ||
  [[ "$(stat -c '%a' "$alternate_bin_dir/dwm-status-ci")" != "755" ]]; then
  echo "Alternate install did not create the requested mode-755 binary" >&2
  exit 1
fi

if [[ "$(stat -c '%a' "$alternate_config_dir/config.toml")" != "644" ]]; then
  echo "Install did not create a mode-644 config" >&2
  exit 1
fi

printf '%s\n' 'keep-existing-config' >"$alternate_config_dir/config.toml"

make --no-print-directory install \
  BINDIR="$alternate_bin_dir" \
  CONFIG_DIR="$alternate_config_dir" \
  BIN="dwm-status-ci"

if [[ "$(<"$alternate_config_dir/config.toml")" != "keep-existing-config" ]]; then
  echo "Repeated install overwrote the existing config" >&2
  exit 1
fi

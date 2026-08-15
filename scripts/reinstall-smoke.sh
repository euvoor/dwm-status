#!/usr/bin/env bash

set -euo pipefail

for required_command in make readlink rustc; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    echo "Missing reinstall smoke dependency: $required_command" >&2
    exit 1
  fi
done

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
smoke_dir=$(mktemp -d)
unrelated_pid=""
replacement_pid=""
trap 'if [[ -n "$replacement_pid" ]]; then kill "$replacement_pid" 2>/dev/null || true; fi; if [[ -n "$unrelated_pid" ]]; then kill "$unrelated_pid" 2>/dev/null || true; wait "$unrelated_pid" 2>/dev/null || true; fi; rm -rf -- "$smoke_dir"' EXIT

install_dir="$smoke_dir/install root/bin"
config_dir="$smoke_dir/install root/config"
log_path="$smoke_dir/state root/dwm_status.log"
installed_bin="$install_dir/dwm_status"
config_path="$config_dir/config.toml"
fixture_source="$smoke_dir/restart_fixture.rs"
fixture_pid_path="$smoke_dir/restart.pid"
unrelated_dir="$smoke_dir/unrelated/bin"
unrelated_bin="$unrelated_dir/dwm_status"
unrelated_pid_path="$smoke_dir/unrelated.pid"

mkdir -p "$install_dir" "$config_dir" "$unrelated_dir"

cat >"$fixture_source" <<'EOF'
use std::{env, fs, io::Write, process, thread, time::Duration};

/// Keeps a disposable process alive unless its config requests failure.
fn main() {
    let config_path = env::args().nth(2).expect("missing config path");
    let pid_path = env::var("RESTART_FIXTURE_PID_FILE").expect("missing PID file");
    let mut pid_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(pid_path)
        .expect("failed to open PID file");
    writeln!(pid_file, "{}", process::id()).expect("failed to record PID");

    if fs::read_to_string(config_path).expect("failed to read config").trim() == "exit" {
        eprintln!("fixture-startup-failure");
        process::exit(23);
    }

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
EOF

printf '%s\n' 'stay' >"$config_path"
rustc --edition 2021 "$fixture_source" -o "$installed_bin"
cp "$installed_bin" "$unrelated_bin"

RESTART_FIXTURE_PID_FILE="$unrelated_pid_path" \
  "$unrelated_bin" --config "$config_path" &
unrelated_pid=$!

if make --no-print-directory -C "$repo_dir" -n reinstall \
  BINDIR="$install_dir" \
  CONFIG_DIR="$config_dir" \
  CONFIG_FILE="$config_path" \
  LOG_FILE="$log_path" >"$smoke_dir/make-dry-run" 2>&1; then
  if ! grep -Fq 'cargo build --release' "$smoke_dir/make-dry-run" ||
    ! grep -Fq 'scripts/restart-installed.sh' "$smoke_dir/make-dry-run"; then
    echo "Reinstall target does not build, install, and restart" >&2
    cat "$smoke_dir/make-dry-run" >&2
    exit 1
  fi
else
  echo "Missing working reinstall target" >&2
  cat "$smoke_dir/make-dry-run" >&2
  exit 1
fi

RESTART_FIXTURE_PID_FILE="$fixture_pid_path" \
  bash "$repo_dir/scripts/restart-installed.sh" \
    "$installed_bin" "$config_path" "$log_path"
replacement_pid=$(tail -n 1 "$fixture_pid_path")

if ! kill -0 "$replacement_pid" 2>/dev/null; then
  echo "Reinstall did not start the installed binary" >&2
  exit 1
fi

if ! kill -0 "$unrelated_pid" 2>/dev/null; then
  echo "Reinstall stopped a same-named binary from another path" >&2
  exit 1
fi

first_pid="$replacement_pid"
rustc --edition 2021 "$fixture_source" -o "$smoke_dir/replacement"
mv "$smoke_dir/replacement" "$installed_bin"

if [[ "$(readlink "/proc/$first_pid/exe")" != *" (deleted)" ]]; then
  echo "Fixture replacement did not produce a deleted executable path" >&2
  exit 1
fi

RESTART_FIXTURE_PID_FILE="$fixture_pid_path" \
  bash "$repo_dir/scripts/restart-installed.sh" \
    "$installed_bin" "$config_path" "$log_path"
replacement_pid=$(tail -n 1 "$fixture_pid_path")

if kill -0 "$first_pid" 2>/dev/null; then
  echo "Reinstall did not stop the pre-install executable" >&2
  exit 1
fi

if [[ "$replacement_pid" == "$first_pid" ]] ||
  ! kill -0 "$replacement_pid" 2>/dev/null; then
  echo "Reinstall did not leave a replacement process running" >&2
  exit 1
fi

printf '%s\n' 'exit' >"$config_path"

if RESTART_FIXTURE_PID_FILE="$fixture_pid_path" \
  bash "$repo_dir/scripts/restart-installed.sh" \
    "$installed_bin" "$config_path" "$log_path"; then
  echo "Reinstall accepted a replacement that failed during startup" >&2
  exit 1
fi

replacement_pid=""

if ! grep -Fq 'fixture-startup-failure' "$log_path"; then
  echo "Reinstall did not retain replacement startup errors" >&2
  exit 1
fi

if ! kill -0 "$unrelated_pid" 2>/dev/null; then
  echo "Reinstall disturbed the unrelated process during failure handling" >&2
  exit 1
fi

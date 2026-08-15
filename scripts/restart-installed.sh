#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 3 ]]; then
  echo "Usage: $0 INSTALLED_BIN CONFIG_FILE LOG_FILE" >&2
  exit 2
fi

installed_bin=$(readlink -f -- "$1")
config_path=$2
log_path=$3

if [[ ! -x "$installed_bin" ]]; then
  echo "Installed binary is not executable: $installed_bin" >&2
  exit 1
fi

if [[ ! -r "$config_path" ]]; then
  echo "Installed config is not readable: $config_path" >&2
  exit 1
fi

install -d "$(dirname -- "$log_path")"
: >"$log_path"

current_uid=$(id -u)
running_pids=()

for process_exe in /proc/[0-9]*/exe; do
  process_pid=${process_exe#/proc/}
  process_pid=${process_pid%/exe}
  process_uid=$(stat -c '%u' "/proc/$process_pid" 2>/dev/null || true)

  if [[ "$process_uid" != "$current_uid" ]]; then
    continue
  fi

  process_bin=$(readlink "$process_exe" 2>/dev/null || true)
  process_bin=${process_bin%" (deleted)"}

  if [[ "$process_bin" == "$installed_bin" ]]; then
    running_pids+=("$process_pid")
  fi
done

for process_pid in "${running_pids[@]}"; do
  if ! kill "$process_pid" 2>/dev/null && kill -0 "$process_pid" 2>/dev/null; then
    echo "Failed to stop $installed_bin (PID $process_pid)" >&2
    exit 1
  fi
done

for _attempt in {1..50}; do
  remaining_pids=()

  for process_pid in "${running_pids[@]}"; do
    if kill -0 "$process_pid" 2>/dev/null; then
      remaining_pids+=("$process_pid")
    fi
  done

  running_pids=("${remaining_pids[@]}")

  if [[ "${#running_pids[@]}" -eq 0 ]]; then
    break
  fi

  sleep 0.1
done

if [[ "${#running_pids[@]}" -ne 0 ]]; then
  echo "Timed out stopping $installed_bin (PIDs: ${running_pids[*]})" >&2
  exit 1
fi

nohup "$installed_bin" --config "$config_path" >"$log_path" 2>&1 </dev/null &
replacement_pid=$!

for _attempt in {1..10}; do
  sleep 0.1

  if ! kill -0 "$replacement_pid" 2>/dev/null; then
    set +e
    wait "$replacement_pid"
    replacement_status=$?
    set -e

    echo "Replacement process failed during startup; log: $log_path" >&2

    if [[ -s "$log_path" ]]; then
      cat "$log_path" >&2
    fi

    if [[ "$replacement_status" -eq 0 ]]; then
      exit 1
    fi

    exit "$replacement_status"
  fi
done

printf 'Restarted %s (PID %s); log: %s\n' \
  "$installed_bin" "$replacement_pid" "$log_path"

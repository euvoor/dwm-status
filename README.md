# dwm_status

[![CI](https://github.com/euvoor/dwm-status/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/euvoor/dwm-status/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`dwm_status` is a Linux/X11 status feeder for `dwm`. It reads local machine state, formats one plain-text line, and replaces `WM_NAME` and `_NET_WM_NAME` on the X root window. It does not draw a bar and does not use a bar protocol.

The root name is framed with `▏` and `▕`. Feature blocks are joined with `▕▏`, empty blocks are skipped, and `features` is rendered in reverse:

```toml
features = ["clock", "cpu"]
```

```text
▏42% ▂▄▆█▕▏Sat 15 Aug 2026 13:45:00 UTC▕
```

Put the feature you want on the right first in `features`. If every enabled feature is empty or unavailable, the payload is `▏▕`.

## Fit

Use `dwm_status` when you want:

- one compiled process that writes the X root name directly;
- built-in Linux samplers configured with strict TOML;
- independent feature scheduling instead of rerunning every block in one fixed shell loop;
- local connectivity state that wakes on route-netlink changes and still performs timed resync;
- startup validation and explicit failure/recovery diagnostics.

It is not a general block plugin host: modules are compiled in, and there is no
click callback interface. Use `slstatus`, `dwmblocks`, or a shell loop instead if
arbitrary commands or click actions are requirements. Use another status tool
for Wayland or portability beyond Linux/X11.

Distribution is currently source-only. The project publishes no tagged
releases, binaries, or package-manager recipes, and compatibility across
untagged commits is not promised. Build the current `master` branch and keep the
commit SHA when reporting a problem.

## Install and start

Build requirements:

- Linux
- Rust 1.85 or newer and Cargo
- Bash
- `make`
- GNU-compatible `install`, `readlink`, `stat`, and `nohup`

```bash
git clone https://github.com/euvoor/dwm-status.git
cd dwm-status
make install
```

The default install writes:

```text
~/.local/bin/dwm_status
~/.config/dwm_status/config.toml
```

The release binary is replaced on every install with mode 755. The config is created with mode 644 only when the destination file does not exist; `make install` never overwrites an existing config.

Start it inside the X session, before `dwm`:

```bash
# ~/.xinitrc
~/.local/bin/dwm_status &
exec dwm
```

### Reinstall and restart

After pulling a new version, rebuild, install, and replace the running user process from a terminal inside the active X session:

```bash
make reinstall
```

`reinstall` runs `install` first, then scans Linux `/proc` for processes owned by the current user whose executable resolves to the installed `BINDIR/BIN` path. This also matches the `(deleted)` executable path held by a process after its binary has been replaced. A same-named binary launched from another path is left alone.

Matching processes receive `SIGTERM`. The command waits up to five seconds for them to stop, then launches the installed binary through `nohup` with `--config "$(CONFIG_FILE)"`. It checks the replacement for one second before returning. The replacement inherits `DISPLAY`, X authority, and the rest of the current environment; `reinstall` does not restart `dwm` or discover another user's X session.

Each run truncates and rewrites `$XDG_STATE_HOME/dwm_status/dwm_status.log`, defaulting to `~/.local/state/dwm_status/dwm_status.log`. Startup failure output is retained there and also printed by `make`. The config is handled by the ordinary non-overwriting `install` target.

The sample glyphs require a Nerd Font-capable `dwm` font:

```c
static const char *fonts[] = { "FiraCode Nerd Font Mono:style=Regular:size=10" };
```

Set any `glyph` to `""` if the font does not contain it.

### Install overrides

The binary prefix and XDG config root are independent.

| Variable | Default | Effect |
|---|---|---|
| `PREFIX` | `$HOME/.local` | Base used by the default `BINDIR`; it does not relocate config. |
| `BINDIR` | `$(PREFIX)/bin` | Binary destination directory. |
| `XDG_CONFIG_HOME` | `$HOME/.config` | Base used by the default `CONFIG_DIR`; it is independent of `PREFIX`. |
| `CONFIG_DIR` | `$(XDG_CONFIG_HOME)/dwm_status` | Config destination directory. |
| `CONFIG_FILE` | `$(CONFIG_DIR)/config.toml` | Config destination path. If it is outside `CONFIG_DIR`, its parent must already exist. |
| `XDG_STATE_HOME` | `$HOME/.local/state` | Base used by the default restart log path. |
| `LOG_FILE` | `$(XDG_STATE_HOME)/dwm_status/dwm_status.log` | Output replaced by each `make reinstall`; its parent is created automatically. |
| `BIN` | `dwm_status` | Destination filename only. Cargo always builds `target/release/dwm_status`. |

For example:

```bash
make install \
  BINDIR="$HOME/bin" \
  CONFIG_DIR="$HOME/.config/dwm_status" \
  BIN=dwm-status
```

A bare `sudo make install` uses root's environment and can target root-owned paths. Pass explicit destinations if installing outside your user account.

If config is installed under a non-default `XDG_CONFIG_HOME`, export the same value when starting `dwm_status` or pass the installed file with `--config`.

Cargo 1.84 and older cannot parse the edition-2024 manifests in the locked dependency graph. They can stop at a dependency-manifest error before reporting this package's declared [`rust-version`](https://doc.rust-lang.org/cargo/reference/rust-version.html). Upgrade Rust instead of regenerating `Cargo.lock`.

## Runtime contract

Required:

- a working X11 session
- `DISPLAY` and matching X authority
- a filesystem Unix-domain X socket or TCP X endpoint; x11rb 0.14 does not try Linux abstract X sockets

Feature-specific:

- `connectivity` opens a route-netlink socket used only for multicast reads; ordinary Linux users do not need `CAP_NET_ADMIN`
- `gpu` runs `nvidia-smi` once per sample and is NVIDIA-only

All other data comes from Linux `/proc`, `/sys`, and `/etc/resolv.conf`.

### Root-window ownership

Each changed final line replaces both properties with the same bytes:

- `WM_NAME`, using the legacy `STRING` property type
- `_NET_WM_NAME`, using `UTF8_STRING`

The two property writes are sequential, not atomic. Separate `xprop` calls during frequent updates can observe adjacent rendered generations.

This is shared mutable X11 state. `xsetroot -name`, another status process, or any other root-name writer races with `dwm_status`; the last writer wins. No external renderer or `xsetroot` subprocess is involved.

Read the values without changing them:

```bash
xprop -root -notype -f WM_NAME 8u WM_NAME
xprop -root -notype -f _NET_WM_NAME 8u _NET_WM_NAME
```

The renderer writes only when the complete payload changes. A failed X write is fatal. The previous root name can remain visible after exit until another writer replaces it.

## Config discovery

Select a file explicitly with either spelling:

```bash
~/.local/bin/dwm_status --config /path/to/config.toml
~/.local/bin/dwm_status -c /path/to/config.toml
```

Without an explicit path, the first existing file wins:

1. `$XDG_CONFIG_HOME/dwm_status/config.toml`, when `XDG_CONFIG_HOME` is set
2. `$HOME/.config/dwm_status/config.toml`, when `HOME` is set
3. `./config.toml` in the process working directory

Duplicate candidate paths are removed. A missing explicit file or a search with no match is a startup error; the error names the attempted path or prints the actual search list.

Config is strict TOML. Unknown top-level or feature keys, unknown feature names, duplicate features, an empty or missing `features` list, invalid value types, an unsupported connectivity format, `connectivity.idle = 0`, an invalid clock format, and an unknown clock timezone all stop startup before X11 is opened. Feature tables can be omitted or partial; omitted keys use the defaults below.

Minimal example:

```toml
# Visual order is CPU, then clock.
features = ["clock", "cpu"]

[clock]
format = "%H:%M"

[cpu]
sparkline_width = 8
```

The checked-in [`config.toml`](config.toml) enables every feature and supplies the sample glyphs.

## Config reference

All type and validation failures below are fatal startup errors. “Recoverable” means the block disappears on the first failed sample, one contextual message is written to stderr, repeated failures stay quiet, and the first later success reports recovery and republishes the block.

| Key | Type and default | Valid values / output effect | Cadence | Failure behavior |
|---|---|---|---|---|
| `features` | required array of strings | Non-empty, unique entries from `connectivity`, `traffic`, `cpu`, `clock`, `ram`, and `gpu`; rendered in reverse list order. | Startup only. | Missing, empty, duplicate, or unknown entries are fatal. |
| `connectivity.glyph` | string, `""` | Prepended verbatim. | Immediate sample, netlink wake, or `idle` tick. | Snapshot reads are recoverable. |
| `connectivity.idle` | unsigned integer, `1` | Slow resync interval in seconds; must be greater than zero. | Timer uses this many seconds and skips missed ticks. | Zero or an invalid integer is fatal. |
| `connectivity.format` | string, `"compact"` | `"compact"` or `"full"`. | Applied on every connectivity render. | Any other value is fatal. |
| `connectivity.show_iface` | boolean, `true` | Show the selected interface name. | Applied on every connectivity render. | Invalid type is fatal. |
| `connectivity.show_route` | boolean, `true` | Show `gw`/`no-gw` or `route:yes`/`route:no`. | Applied on every connectivity render. | Invalid type is fatal. |
| `connectivity.show_dns` | boolean, `true` | Show resolver classification. | Applied on every connectivity render. | Invalid type is fatal. |
| `connectivity.show_kind` | boolean, `true` | Show the interface class. | Applied on every connectivity render. | Invalid type is fatal. |
| `clock.glyph` | string, `""` | Prepended verbatim to the trimmed formatted value. | Immediate, then next visible format boundary. | Invalid type is fatal. |
| `clock.format` | string, `"%a %d %b %Y %X %Z"` | A [Chrono strftime format](https://docs.rs/chrono/0.4.45/chrono/format/strftime/index.html). | Second, minute, or local-date boundary, inferred from parsed directives. | Invalid directives are fatal. |
| `clock.timezone` | string, `""` | Empty means machine local time; otherwise a [chrono-tz name](https://docs.rs/chrono-tz/0.10.4/chrono_tz/enum.Tz.html), such as `Europe/Berlin` or `UTC`. | Applied on every clock render, including DST transitions. | Unknown names are fatal. |
| `ram.glyph` | string, `""` | Prepended verbatim. | Immediate, then every second. | `/proc/meminfo` reads and parsing are recoverable. |
| `cpu.glyph` | string, `""` | Prepended verbatim. | Immediate, then every second. | `/proc/stat` reads and parsing are recoverable; missing temperature is not a failure. |
| `cpu.sparkline_width` | unsigned integer, `8` | `0` means one glyph per logical CPU; a positive value caps width by averaging buckets when there are more cores than columns. | Applied on every CPU render. | Invalid type is fatal. |
| `gpu.glyph` | string, `""` | Prepended verbatim. | Immediate, then every second. | Command, exit-status, encoding, and required-row failures are recoverable. |
| `traffic.glyph` | string, `""` | Prepended verbatim. | Immediate, then every second. | Route, sysfs, and procfs reads are recoverable; no selected interface yields an empty block. |

## Feature semantics

### `connectivity`

Typical compact output:

```text
W:wlp4s0 gw stub
T:wg0 gw dns
E:enp10s0 no-gw dns
```

Compact tokens:

- `W`: `/sys/class/net/IFACE/wireless` exists
- `T`: the interface name begins with `wg`, `tun`, `tap`, `ppp`, `tailscale`, or `zt`
- `E`: fallback class for a non-loopback interface that is neither detected wireless nor matched as a tunnel; it is not proof of physical Ethernet
- `N`: other
- `gw` / `no-gw`: a usable default route does / does not exist on the selected up interface
- `dns`: one or more direct resolver addresses
- `stub`: loopback resolver addresses only
- `mixed`: loopback and direct resolver addresses
- `no-dns`: no valid resolver address

Full mode uses labeled values such as:

```text
kind:wifi iface:wlp4s0 link:up route:yes dns:stub
```

Kind labels are `wifi`, `ethernet`, `tunnel`, and `other`. With no selected interface, compact mode emits `no-iface` and full mode emits `iface:none`. The no-interface sentinel remains visible even when interface/kind fields are disabled.

Behavior and data sources:

- It sends no packets and contacts no remote host. It is local state, not proof of Internet reachability.
- It reads IPv4 defaults from `/proc/net/route`, IPv6 defaults from `/proc/net/ipv6_route`, interface names and link traits from `/sys/class/net`, counters from `/proc/net/dev`, and resolvers from `/etc/resolv.conf`.
- A route must have a zero destination/prefix, be marked up, not be marked reject, and refer to an up interface.
- The lowest metric wins. Ties use interface name, then IPv4 before IPv6.
- Without a usable default, the first alphabetically named up, non-loopback interface is selected and the route token reports no gateway.
- Carrier `1` is treated as up when a carrier file exists. Otherwise `operstate` values `up` and `unknown` are treated as up.
- Resolver parsing accepts a valid IP address as the second field of a `nameserver` directive. Comments, malformed addresses, and other directives are ignored.
- A read-only route-netlink listener subscribes to link, IPv4/IPv6 address, and IPv4/IPv6 route multicast groups. Bursts coalesce into wakeups.
- DNS has no kernel event subscription. `idle` supplies DNS refresh, slow resync, and missed-event recovery.
- If the listener cannot open or later stops, one diagnostic is printed and timed resync continues for the rest of that process. The listener is not retried.

### `traffic`

```text
󰖟 ↓12.4M ↑1.1M
```

- `↓` is receive rate and `↑` is transmit rate.
- Values are bytes per second. Labels `B`, `K`, `M`, `G`, `T`, and `P` use powers of 1024; the output omits the literal `/s`.
- It reads `/proc/net/dev` and uses the same route/interface selection as `connectivity`.
- The first sample and a primary-interface change produce zero rates. A decreasing/reset counter holds that direction at zero. A missing selected interface produces an empty block.
- Rates use actual elapsed monotonic time between samples.

### `cpu`

```text
 42% ▁▂▄▇ 55°
```

- Usage and the sparkline come from one `/proc/stat` snapshot.
- Usage is `100 × (Δtotal - Δidle) / Δtotal`, clamped to 0–100 and rounded to a whole percent.
- `total` counts user through steal once; `idle` is idle plus iowait. Guest and guest-nice are excluded because Linux already includes them in user and nice.
- The first sample compares the kernel counters with zero, so it represents boot-to-date average CPU use. Later samples are one-second deltas.
- The sparkline uses `▁▂▃▄▅▆▇█`; `█` is the 100% ceiling.
- `sparkline_width = 0` renders one glyph per logical CPU. A positive width averages contiguous core buckets only when the core count exceeds that width.
- Temperature is read directly from plausible CPU/package sensors under `/sys/class/hwmon`, falling back to `/sys/class/thermal`. Values not strictly between 0°C and 150°C are ignored.
- If no CPU temperature can be identified, only the temperature suffix disappears. No external sensor command is run.

### `ram`

```text
󰍛 5.3G · 34%
```

- Values come from one [`/proc/meminfo` snapshot](https://docs.kernel.org/filesystems/proc.html#meminfo).
- Used RAM is `MemTotal - MemAvailable`.
- If `MemAvailable` is absent, available RAM is `MemFree + Buffers + Cached + SReclaimable`.
- Kernel `kB` values are multiplied by 1024.
- Byte labels `B`, `K`, `M`, `G`, `T`, and `P` use powers of 1024. The byte value has one decimal above bytes; percentage is rounded to a whole number.

### `gpu`

```text
󰢮 23% · V: 18% · T: 47° · F: 32%
```

- This feature is NVIDIA-only.
- Every sample runs:

```bash
nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu,fan.speed --format=csv,noheader,nounits
```

- Only the first non-empty GPU row is used.
- The leading percentage is `utilization.gpu`.
- `V:` is [frame-buffer occupancy](https://docs.nvidia.com/deploy/nvidia-smi/index.html#fb-memory-usage): `memory.used / memory.total`, rounded to the nearest whole percent and clamped to 0–100. It is not NVIDIA memory-bus utilization.
- `T:` is GPU temperature in Celsius and `F:` is fan percentage.
- Unsupported, malformed, or zero-capacity optional fields are omitted.
- If GPU utilization itself is unsupported or malformed, the block is empty. A missing command, nonzero command exit, invalid output encoding, missing row, or incomplete row uses the recoverable failure policy.

### `clock`

- Formatting uses [Chrono strftime semantics](https://docs.rs/chrono/0.4.45/chrono/format/strftime/index.html).
- The formatted value is trimmed before `glyph` is prepended.
- `timezone = ""` uses the machine timezone. A non-empty value uses the named chrono-tz zone.
- `%Z` reports `UTC` for a zero offset that Chrono formats numerically.
- Seconds, timestamps, fractional seconds, RFC formats, and aliases containing them wake at the next second boundary.
- Hour, minute, AM/PM, offset, and timezone fields wake at the next minute boundary.
- Date-only and literal formats wake at the first representable instant of the next local date, including offset transitions that remove midnight.

## Failure policy

Config, clock format/timezone validation, and the initial X11 connection are startup-fatal.

Connectivity snapshots, traffic, CPU, RAM, and GPU use transition-only health reporting:

1. the first failed sample clears that feature block and prints `Feature NAME failed: ...`
2. repeated failures print nothing
3. the first successful sample restores the block and prints `Feature NAME recovered`

A feature worker panic, a feature worker returning, all workers stopping, or any root-property write/flush failure is fatal. The process prints context and exits nonzero. Use a session supervisor if automatic restart is required.

## Troubleshooting

### `Failed to connect to X11`

Run `dwm_status` as the same user and inside the same session as `dwm`. Check:

```bash
printf '%s\n' "$DISPLAY"
xprop -root WM_NAME
```

Do not start the status process through `sudo`; root usually lacks the session's X authority. A sandbox exposing only a Linux abstract X socket is insufficient for x11rb 0.14.

### `make reinstall` fails

Read the log path printed by the command. The usual causes are a missing `DISPLAY`, wrong X authority, or an invalid installed config. If termination times out, no replacement is started. A feeder launched from a different binary path is deliberately not stopped; stop it separately before using the installed copy.

### Missing or wrong config

Use `--config` with an absolute path. Without it, the startup error prints the real search list. TOML errors include the selected path and offending key/value.

### GPU block disappears

Run the documented `nvidia-smi` command directly. If the machine is not using the NVIDIA driver stack, remove `"gpu"` from `features`. The first failure is on stderr.

### CPU temperature is absent

CPU load still works. The kernel did not expose a sensor that matched the CPU/package heuristics. Inspect `/sys/class/hwmon` and `/sys/class/thermal`; no external `sensors` fallback exists.

### Connectivity updates only on `idle`

Look for `Feature connectivity event stream ...; using timed resync` on stderr. Route-netlink may be denied by a container or sandbox, or the listener may have stopped. Timed refresh continues and no listener retry occurs.

### Glyphs are boxes or missing

Configure a Nerd Font-capable `dwm` font, or clear the glyphs. The program writes UTF-8 bytes but cannot make the Xft font contain those code points.

### Blocks appear in the wrong order

`features` is intentionally rendered in reverse. To display `A B C` from left to right, configure `["C", "B", "A"]`.

### The root name changes back

Another process owns the same root-window properties. Stop `xsetroot -name`, shell loops, and other status feeders. Use the read-only `xprop` commands above to observe the race.

## Development and verification

`make` runs the release binary with `./config.toml`. It requires a live X session and writes the real root-window name.

`make dev` does the same through `cargo-watch`:

```bash
cargo install cargo-watch
make dev
```

`cargo-watch` is a development dependency, not a runtime dependency.

Safe local quality commands:

```bash
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
./scripts/install-smoke.sh
./scripts/reinstall-smoke.sh
```

Parser and formatter tests use checked-in fixtures rather than the developer machine's live routes, resolver, counters, sensors, or GPU. The install and reinstall smokes use `mktemp` trees and do not touch `$HOME`, the live X session, or an installed status process. The reinstall smoke compiles a disposable process fixture to verify exact executable matching, deleted-inode replacement, detachment, and startup failure reporting.

Additional Linux integration checks:

```bash
./scripts/xvfb-smoke.sh
./scripts/netlink-smoke.sh
```

The X11 smoke needs `Xvfb` and `xprop`. It writes only a temporary X server, then kills that server and requires the status process to fail on its next root write.

The netlink smoke also needs `ip`, `mount`, `unshare`, and non-interactive `sudo`. It creates link/route state only inside disposable network and mount namespaces, uses a temporary Xvfb server, and does not change host routes or mounts.

CI runs:

- locked tests and a release build on exact Rust 1.85.0
- locked tests, strict Clippy, release build, isolated install/reinstall smokes, X11 smoke, and netlink smoke on latest stable Rust
- a RustSec audit of `Cargo.lock`

On this repository's project board, Done means the issue is merged into `develop` and all CI jobs pass on the exact merge commit. It does not claim manual validation on every GPU, sensor layout, X server, font, or Linux distribution. `master` remains owner-controlled.

## Contributing and support

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before opening code or documentation
changes. Pull requests target `develop`; maintainers promote tested work to
`master` separately.

Use the [issue chooser](https://github.com/euvoor/dwm-status/issues/new/choose)
for reproducible bugs, concrete feature proposals, and focused usage questions.
Read [`SUPPORT.md`](SUPPORT.md) for the required environment and diagnostic
details. Report suspected vulnerabilities privately as described in
[`SECURITY.md`](SECURITY.md), and follow
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) in all project spaces.

## License

`dwm_status` is available under the [MIT License](LICENSE).

# dwm_status

`dwm_status` is a small Rust status feeder for `dwm`.

It does one thing: collect local machine state, assemble a plain text status line, and push it into the X root window with `xsetroot`.

The design target is the usual `dwm` setup:

- plain text instead of a bar protocol
- one config file you can hand-edit
- local state first
- no active network probing by default
- no extra daemon layer just to draw text

The current tree is Linux/X11-specific. That is intentional.

## Current feature set

- `connectivity`
- `date_time`
- `memory`
- `cpu`
- `gpu`
- `net_stats`

`connectivity` is passive. It reads kernel and resolver state, but it does not send packets.

Rendering is event-driven. Features publish their own updates, and the root name is only rewritten when the final bar string changes.

## Install

Build it:

```bash
cargo build --release
```

The binary currently reads `config.toml` from its working directory, so install the binary and the config together.

One straightforward setup:

```bash
install -d ~/.local/lib/dwm_status ~/.local/bin
install -m755 target/release/dwm_status ~/.local/lib/dwm_status/dwm_status
install -m644 config.toml ~/.local/lib/dwm_status/config.toml

cat > ~/.local/bin/dwm_status <<'EOF'
#!/usr/bin/env bash
cd "$HOME/.local/lib/dwm_status" || exit 1
exec ./dwm_status
EOF

chmod +x ~/.local/bin/dwm_status
```

Then start it from your `dwm` session:

```bash
~/.local/bin/dwm_status &
```

Typical place:

```bash
# ~/.xinitrc
~/.local/bin/dwm_status &
exec dwm
```

If `config.toml` is missing or invalid, the process prints an error and exits.

## Runtime dependencies

Required:

- `xsetroot`

Feature-specific:

- `cpu`: `sensors` from `lm_sensors` if you want temperature output
- `gpu`: `nvidia-smi`
- `gpu`: `nvidia-settings`

Everything else is read from Linux interfaces such as `/proc`, `/sys`, and `/etc/resolv.conf`.

## Config

Config is TOML. Keep a full file, comment out what you do not need, and keep going.

Example:

```toml
# Current code renders the final bar in reverse feature order.
features = [
  "connectivity",
  "date_time",
  "memory",
  "cpu",
  "gpu",
  "net_stats",
]

[connectivity]
prefix = ""
idle = 1
format = "compact"
show_iface = true
show_route = true
show_dns = true
show_kind = true

[date_time]
prefix = ""
idle = 1
format = "%a %d %b %Y %X %Z"

[memory]
prefix = ""
idle = 1
output = "used"

[cpu]
prefix = ""
idle = 1
chip = "k10temp-pci-00c3"
report = "Tctl"

[gpu]
prefix = ""
idle = 1

[net_stats]
prefix = ""
idle = 1
ifaces = [
  "enp10s0",
  "wlx04d4c464bd3c",
]
```

## Output format

The bar writer wraps the whole line with `▏` and `▕`, and joins enabled feature outputs with `▕▏`.

So if two features emit `A` and `B`, the root name becomes:

```text
▏B▕▏A▕
```

That reversal is current behavior, not documentation drift.

The renderer itself is no longer on a fixed one-second loop. Redraws happen when a feature publishes a new value.

## Symbol reference

### Connectivity

Compact mode uses short labels:

- `W` = wireless interface
- `E` = ethernet interface
- `T` = tunnel-style interface
- `N` = other interface type
- `gw` = default route exists
- `no-gw` = no default route found
- `dns` = direct resolver addresses in `resolv.conf`
- `stub` = loopback resolver only, for example `127.0.0.53`
- `mixed` = both loopback and direct resolver entries found
- `no-dns` = no `nameserver` lines found
- `down` = selected primary interface is down

Typical compact output:

```text
W:wlp4s0 gw stub
T:wg0 gw dns
E:enp10s0 no-gw dns down
```

### Net Stats

Per-interface traffic labels reuse the same kind markers:

- `W:` = wireless
- `E:` = ethernet
- `T:` = tunnel
- `N:` = other

Example:

```text
(W: 12.4 MiB/1.1 MiB) (E: 0 B/0 B)
```

### CPU

The per-core sparkline uses:

```text
▁▂▃▄▅▆▇█▉
```

Left to right means low to high per-core activity.

## Feature notes

### `connectivity`

This is the privacy-first network feature.

- It does not ping anything.
- It does not talk to third-party hosts.
- It reports local state only.
- It picks a primary interface from the default route when possible.
- Tunnel detection is generic and based on interface naming patterns such as `wg*`, `tun*`, `tap*`, `ppp*`, `tailscale*`, and `zt*`.

Use it as an honest local indicator, not as proof that the wider Internet is reachable.

### `date_time`

- Uses `chrono` formatting.
- Current code uses `UTC`, not local time.

### `memory`

- `output` supports `used`, `free`, or `percentage`.
- Values come from `/proc/meminfo`.

### `cpu`

- Usage comes from `/proc/stat`.
- Load and thread count come from `/proc/loadavg`.
- Temperature currently parses the `Composite` line from `sensors`.
- `chip` and `report` are still in the config, but current code does not use them yet.

### `gpu`

- NVIDIA-only today.
- Reads utilization and temperature from `nvidia-smi`.
- Reads fan RPM from `nvidia-settings`.
- Also writes fan speed with `nvidia-settings`.

So this feature is not just telemetry. It actively pushes fan control.

### `net_stats`

- Reads `/proc/net/dev`.
- Shows per-interval RX/TX deltas, not lifetime counters.
- Reuses the same interface-kind detection as `connectivity`.

## Current constraints

- Unsupported feature names still panic.
- Missing optional commands can make a feature lose part of its output.
- `gpu` is intentionally opinionated and machine-specific.
- The status line is currently rendered in reverse `features` order.

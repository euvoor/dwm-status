# dwm_status

`dwm_status` is a small Rust status feeder for `dwm`.

It does one thing: collect local machine state, assemble a plain text status line, and push it into the X root window name over X11.

The design target is the usual `dwm` setup:

- plain text instead of a bar protocol
- one config file you can hand-edit
- local state first
- no active network probing by default
- no extra daemon layer just to draw text

The current tree is Linux/X11-specific. That is intentional.

## Current feature set

- `connectivity`
- `clock`
- `memory`
- `cpu`
- `gpu`
- `net_stats`

`connectivity` is passive. It reads kernel and resolver state, but it does not send packets.

Rendering is event-driven. Features publish their own updates, and the root name is only rewritten when the final bar string changes.

Periodic sampler features now run on a fixed cadence instead of `work + sleep` drift.

## Install

Build it:

```bash
cargo build --release
```

One straightforward setup:

```bash
install -d ~/.local/bin ~/.config/dwm_status
install -m755 target/release/dwm_status ~/.local/bin/dwm_status
install -m644 config.toml ~/.config/dwm_status/config.toml
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

Config lookup order is:

1. `--config /path/to/config.toml`
2. `$XDG_CONFIG_HOME/dwm_status/config.toml`
3. `~/.config/dwm_status/config.toml`
4. `./config.toml`

If the selected config is missing or invalid, the process prints an error and exits.

## Runtime dependencies

Required:

- a working X11 session with `DISPLAY` and X authority available

Feature-specific:

- `cpu`: `sensors` from `lm_sensors` if you want temperature output
- `gpu`: `nvidia-smi`
- `gpu`: `nvidia-settings`

Everything else is read from Linux interfaces such as `/proc`, `/sys`, and `/etc/resolv.conf`.

No external renderer command is required now. The binary talks to X directly.

## Config

Config is TOML. Keep a full file, comment out what you do not need, and keep going.

You can point to a specific file with:

```bash
dwm_status --config /path/to/config.toml
```

Example:

```toml
# Current code renders the final bar in reverse feature order.
features = [
  "connectivity",
  "clock",
  "memory",
  "cpu",
  "gpu",
  "net_stats",
]

[connectivity]
glyph = "󰖩 "
idle = 1
format = "compact"
show_iface = true
show_route = true
show_dns = true
show_kind = true

[clock]
glyph = " "
format = "%a %d %b %Y %X %Z"
timezone = ""

[memory]
glyph = ""
idle = 1
output = "used"

[cpu]
glyph = ""
idle = 1
chip = "k10temp-pci-00c3"
report = "Tctl"

[gpu]
glyph = ""
idle = 1

[net_stats]
glyph = ""
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
`connectivity` now wakes on kernel route and link events, and uses `idle` as its slow resync interval.

The sample `clock` and `connectivity` glyphs assume a Nerd Font-capable status font.
If your bar font does not carry them, replace them or set `glyph = ""`.

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
- The sample config uses `󰖩 ` as its glyph.
- It wakes on kernel route, address, and link changes when netlink is available.
- `idle` is the fallback resync interval for DNS changes and missed events.
- It picks a primary interface from the default route when possible.
- Tunnel detection is generic and based on interface naming patterns such as `wg*`, `tun*`, `tap*`, `ppp*`, `tailscale*`, and `zt*`.

Use it as an honest local indicator, not as proof that the wider Internet is reachable.

### `clock`

- Uses `chrono` formatting.
- The sample config uses ` ` as its glyph.
- Defaults to the machine's local time.
- `timezone = ""` uses the machine's local timezone.
- Any non-empty `timezone` must be a named zone such as `Europe/Berlin` or `UTC`.
- Refresh cadence comes from the format itself.
- Second-based formats wake every second, minute-only formats wake every minute, and date-only formats wake at the next midnight in the selected timezone.

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

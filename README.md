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
- `ram`
- `cpu`
- `gpu`
- `traffic`

`connectivity` is passive. It reads kernel and resolver state, but it does not send packets.

Rendering is event-driven. Features publish their own updates, and the root name is only rewritten when the final bar string changes.

Periodic sampler features now run on a fixed cadence instead of `work + sleep` drift.

## Install

Build and install it:

```bash
make install
```

This installs:

```bash
~/.local/bin/dwm_status
~/.config/dwm_status/config.toml
```

The binary is replaced on each install. An existing config file is left untouched.
Building requires Rust, Cargo, `make`, and the standard `install` utility.

If you use the sample glyphs, set a Nerd Font-capable status font in `dwm`.
For example:

```c
static const char *fonts[] = { "FiraCode Nerd Font Mono:style=Regular:size=10" };
```

Then start it from your `dwm` session:

```bash
~/.local/bin/dwm_status &
```

Local workflows:

```bash
make
```

Runs the release build from the current directory with `./config.toml`.

```bash
make dev
```

Runs the local dev loop from the current directory.
This target uses `cargo watch`.

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

- `gpu`: `nvidia-smi`

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
  "ram",
  "cpu",
  "gpu",
  "traffic",
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

[ram]
glyph = "󰍛 "

[cpu]
glyph = " "
sparkline_width = 0

[gpu]
glyph = "󰢮 "

[traffic]
glyph = "󰖟 "
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

The sample `connectivity`, `clock`, `ram`, `cpu`, `gpu`, and `traffic` glyphs assume a Nerd Font-capable status font.
For `dwm`, a matching line in `config.h` is:

```c
static const char *fonts[] = { "FiraCode Nerd Font Mono:style=Regular:size=10" };
```

If your bar font still does not carry them, replace them or set `glyph = ""`.

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

### Traffic

- `↓` = receive rate
- `↑` = transmit rate

Example:

```text
󰖟 ↓12.4M ↑1.1M
```

### CPU

The CPU sparkline uses:

```text
▁▂▃▄▅▆▇█▉
```

Left to right means low to high CPU activity, either per core or grouped by config.

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

### `ram`

- Renders `used · used%` as one compact line.
- The sample config uses `󰍛 ` as its glyph.
- Uses a fixed internal cadence instead of a config knob.
- Values come from `/proc/meminfo`.

### `cpu`

- Renders `usage% sparkline temp` as one compact line.
- The sample config uses ` ` as its glyph.
- Uses a fixed internal one-second cadence.
- Usage comes from `/proc/stat`.
- `sparkline_width = 0` renders one glyph per logical CPU.
- Any positive `sparkline_width` groups the sparkline to that many columns.
- Temperature is read directly from `/sys/class/hwmon` and `/sys/class/thermal` when the kernel exposes a sane CPU sensor.
- If no CPU temperature can be identified, the block omits it instead of shelling out to a tool.

### `gpu`

- NVIDIA-only today.
- Renders `usage% · V: vram% · T: temp° · F: fan%` as one compact line.
- The sample config uses `󰢮 ` as its glyph.
- Uses a fixed internal one-second cadence.
- Reads telemetry through `nvidia-smi --query-gpu=... --format=csv,noheader,nounits`.
- Uses the first GPU row reported by `nvidia-smi`.
- Drops unsupported fields such as fan speed instead of printing noisy placeholders.

### `traffic`

- Renders `↓recv ↑trans` as one compact line.
- The sample config uses `󰖟 ` as its glyph.
- Uses a fixed internal one-second cadence.
- Reads counters from `/proc/net/dev`.
- Follows the current primary routed interface automatically.
- Resets cleanly when the primary interface changes.

## Current constraints

- Unsupported feature names print an error with the supported names and exit before connecting to X11.
- Missing optional commands can make a feature lose part of its output.
- `gpu` is still NVIDIA-specific.
- The status line is currently rendered in reverse `features` order.

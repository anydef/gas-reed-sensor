# Embassy setup & running examples

Initial toolchain setup, debug probe setup, and how to build/flash embassy's own
examples on the Pico W. See the main [README](README.md) for project status.

## Prerequisites

### 1. Embassy checkout

`embassy/` is a plain clone, not a submodule, and it's listed in `.gitignore`. Get it with:

```sh
git clone https://github.com/embassy-rs/embassy.git gas-reed-rs/embassy
```

### 2. Rust toolchain and the `thumbv6m-none-eabi` target

The RP2040 is a Cortex-M0+, so builds need the `thumbv6m-none-eabi` target. If it
isn't available you get:

```
error[E0463]: can't find crate for `core`
  = note: the `thumbv6m-none-eabi` target may not be installed
```

**Two entirely different causes produce that identical error on this machine.**
Diagnose which one you're hitting before doing anything else:

```sh
rustc --print target-libdir --target thumbv6m-none-eabi
```

| Output | Cause |
| --- | --- |
| `/usr/lib/rustlib/...` | **Cause B** — the distro rustc is shadowing rustup |
| `~/.rustup/toolchains/<tc>/lib/rustlib/...` but build still fails | **Cause A** — target missing from that toolchain |

#### Cause A — target not installed in the mise-selected toolchain

`embassy/rust-toolchain.toml` pins channel `1.97` and lists every target embassy
supports, so *normally* rustup installs the target automatically on first build.

That auto-install does not happen here. The root `mise.toml` pins `rust = "1.97"`,
and mise implements that by exporting `RUSTUP_TOOLCHAIN=1.97.1` into the shell.
That environment variable takes priority over `rust-toolchain.toml`, and rustup
only honours the file's `targets = [...]` list when the file is what selects the
toolchain. With the env var winning you get a bare `1.97.1` toolchain carrying
only the host target.

Fix it either way:

**Option A1 — install the target into the mise-managed toolchain** (keeps mise in charge):

```sh
rustup target add thumbv6m-none-eabi --toolchain 1.97.1-x86_64-unknown-linux-gnu
rustup component add llvm-tools rust-src --toolchain 1.97.1-x86_64-unknown-linux-gnu
```

This is per-toolchain — repeat it whenever `mise.toml` bumps the Rust version.

**Option A2 — let embassy's `rust-toolchain.toml` govern**: drop the `rust` entry
from the root `mise.toml`. rustup then reads the file and installs all of its
targets and components on demand, with no manual step.

#### Cause B — Arch/Manjaro's `rust` package shadowing rustup

If `/usr/bin/rustc` (pacman's `rust` package) comes before `~/.cargo/bin/rustc`
in `PATH`, the rustup shim is never invoked. The distro rustc uses
`/usr/lib/rustlib`, has no bare-metal targets, and `rustup target add` cannot add
any to it — so the build fails no matter how many times you install the target.

Confirm with:

```sh
type -a rustc          # /usr/bin/rustc listed first == shadowed
```

The tell-tale symptom is a **full rebuild from scratch** (starting at `Compiling
proc-macro2`) even when `target/` is already populated — a different rustc
invalidates every fingerprint.

Prepending `~/.cargo/bin` to `PATH` by hand does not stick: mise's zsh `precmd`
hook rewrites `PATH` on every prompt and discards the edit. Remove the distro
package instead:

```sh
sudo pacman -Rs rust
```

It's safe — `Required By: None`, and it's only an *optional* dep of `kate` (for
Rust LSP, which the rustup toolchain provides anyway). Recovers ~305 MiB.

#### Verify

```sh
rustup target list --installed                              # lists thumbv6m-none-eabi
rustc --print target-libdir --target thumbv6m-none-eabi     # points into ~/.rustup
```

### 3. Debug probe

Flashing needs `probe-rs` (`cargo install probe-rs-tools`) plus a probe — the
Pico W has no on-board debug hardware. Everything below assumes the official
**Raspberry Pi Debug Probe**. There's a probe-less fallback under
[Flashing over USB](#over-usb-no-probe), but it's a poor way to work here.

#### Wiring

Use the **D** port on the probe (`U` is the UART passthrough). The kit's
three-wire JST-SH cable is colour-coded:

| Wire | Signal | Pico W pad |
| --- | --- | --- |
| Orange | SWCLK | `SWCLK` |
| Black | GND | `GND` |
| Yellow | SWDIO | `SWDIO` |

On a 2022 Pico W the three debug pads are on the **bottom edge** of the board,
opposite the USB connector, each labelled in silkscreen. Go by the labels, not by
position. Swapping SWDIO/SWCLK just fails to connect, nothing is damaged; GND is
the one to get right.

**The probe does not power the target.** The `D` port carries only SWCLK, GND and
SWDIO, so the Pico W needs its own 5V — its own USB cable to a charger, hub or
PC. A silently unpowered target is the most common "target not responding" cause.
The probe itself must go to the PC running `probe-rs`; the Pico W's cable can go
anywhere that supplies power.

#### udev (Linux)

`69-probe-rs.rules` — installed by probe-rs — ends with a catch-all:

```
ATTRS{product}=="*CMSIS-DAP*", MODE="660", GROUP="plugdev", TAG+="uaccess"
```

The Debug Probe's USB product string is `Debug Probe (CMSIS-DAP)`, so it matches
and **no `2e8a` rule needs adding**. The `GROUP="plugdev"` reference is inert on
Manjaro (no such group); `TAG+="uaccess"` is what actually grants the logged-in
session an ACL on the device node.

Check it took:

```sh
lsusb | grep 2e8a      # expect 2e8a:000c
probe-rs list          # expect "Debug Probe (CMSIS-DAP)"
```

If `lsusb` sees it but `probe-rs list` doesn't, reload rules
(`sudo udevadm control --reload && sudo udevadm trigger`) and replug.

#### Probe firmware

probe-rs 0.32 requires CMSIS-DAP firmware **≥ 2.2.0**. Probes from 2022 ship with
1.01 and fail with:

```
Error: Failed to open probe: ...
    2: The firmware on the probe is outdated, and not supported by probe-rs.
       The minimum supported firmware version is 2.2.0.
```

Check the installed version — `bcdDevice` is the firmware revision:

```sh
lsusb -d 2e8a: -v 2>/dev/null | grep bcdDevice
```

To update, grab `debugprobe.uf2` from
[raspberrypi/debugprobe releases](https://github.com/raspberrypi/debugprobe/releases)
(v2.3.1 at time of writing). Take plain `debugprobe.uf2` — `debugprobe_on_pico.uf2`
is for a spare Pico acting as a probe, which is different hardware.

1. Unplug the probe.
2. Hold its **BOOTSEL** button and plug it back in. The button is on the probe's
   own board — the case snaps apart by hand if you can't reach it.
3. It mounts as `RPI-RP2`.
4. Copy the `.uf2` onto it; it reboots into the new firmware.

The probe keeps its `2e8a:000c` ID across the update, so udev rules stay valid.

## Building the examples

```sh
cd embassy/examples/rp
cargo build --bin wifi_blinky --release
```

The `.cargo/config.toml` in that directory already sets
`target = "thumbv6m-none-eabi"`, so no `--target` flag is needed.

### Which example to start with

Use **`wifi_blinky`**, not `blinky`. On the plain Pico the LED is on GPIO25, which
is what `blinky` drives; on the **Pico W** that pin is wired to the CYW43439 wifi
chip and the LED hangs off the wifi module instead. `wifi_blinky` therefore also
exercises the CYW43 firmware upload, which is the prerequisite for everything else:

| Example | What it proves |
| --- | --- |
| `wifi_blinky` | CYW43 firmware loads; LED toggles via the wifi chip |
| `wifi_scan` | Radio works, sees access points |
| `wifi_tcp_server` | Joins your network, gets DHCP, serves TCP |
| `wifi_webrequest` | Outbound HTTP — closest to what this port needs for Pushgateway |

`wifi_blinky` and `wifi_scan` need no credentials. `wifi_tcp_server` and
`wifi_webrequest` hardcode them as consts near the top of the file, which you have
to edit before building:

```rust
const WIFI_NETWORK: &str = "ssid"; // change to your network SSID
const WIFI_PASSWORD: &str = "pwd"; // change to your network password
```

Since `embassy/` is gitignored these edits aren't tracked, but they are still
credentials sitting in a source file — worth replacing with `env!()` reads once
this moves into `gas-reed-rs` proper, mirroring how `env.conf` works on the
MicroPython side.

## Flashing

### With the debug probe

```sh
cargo run --bin wifi_blinky --release
```

`.cargo/config.toml` already sets `runner = "probe-rs run --chip RP2040"`, correct
for the Pico W. This flashes over SWD and then streams `defmt` logs back over RTT,
with real panic messages.

### Over USB (no probe)

1. Hold **BOOTSEL** while plugging the Pico in. It enumerates as a USB mass
   storage device — confirm with `lsusb | grep 2e8a`. If nothing shows up, suspect
   a power-only USB cable before anything else.
2. Flash the built ELF:

   ```sh
   elf2uf2-rs -d target/thumbv6m-none-eabi/release/wifi_blinky
   ```

   `-d` writes the `.uf2` straight to the mounted board, which then reboots into
   the new firmware.

To make this the default `cargo run` behaviour, change the runner in
`embassy/examples/rp/.cargo/config.toml`:

```toml
[target.'cfg(all(target_arch = "arm", target_os = "none"))']
runner = "elf2uf2-rs -d"
```

**Caveat:** the examples log through `defmt-rtt` and panic through `panic-probe`,
both of which need a probe attached to say anything. Over plain USB you get no log
output and no panic messages — you can see `wifi_blinky` blink, but `wifi_scan`
and friends will appear to do nothing.

## Troubleshooting quick reference

| Symptom | Cause |
| --- | --- |
| `can't find crate for core`, target *is* installed | distro rustc shadowing rustup — see Cause B |
| `can't find crate for core`, full rebuild from scratch | same — different rustc invalidates fingerprints |
| `can't find crate for core`, rustup path correct | target missing from the mise toolchain — Cause A |
| `No connected probes were found` | probe not plugged into the PC, or udev/permissions |
| `The firmware on the probe is outdated` | probe firmware < 2.2.0 — reflash `debugprobe.uf2` |
| Probe connects, target not responding | Pico W has no power of its own — the `D` port supplies none |
| `blinky` builds and flashes but no LED | wrong example for a Pico W — use `wifi_blinky` |

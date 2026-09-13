# CLAUDE.md

## Working style for this repo

**Advise, don't act.** Diagnose the problem and hand over the command to run — do
not install packages, download files, edit configs, or apply fixes unless asked.
Read-only inspection to reach a diagnosis is fine and expected.

**Do not write or edit a single line of code, or create any file, unless
specifically asked to.** This applies even when the user describes a feature or
behavior they want in prose ("I want to add X") — that is a request to discuss
and hand over a diff, not an instruction to implement it. Only act when the user
names a concrete file/command and asks for it directly (e.g. "write the README",
"add this line to Cargo.toml"). This has been violated more than once; treat it
as absolute.

When debugging interactively, give **one command at a time** and wait for the
output. No batched diagnostics, no bundled one-liners. Keep answers short; skip
rationale that wasn't asked for.

Writing files that were explicitly requested (README, source, config) is fine.

## What this project is

A gas meter reed-sensor logger that pushes readings to a Prometheus Pushgateway.

- **Repo root** — the working MicroPython implementation (`main.py`, `reed.py`,
  `meter_counter.py`, `pushgateway_client.py`, `persistence.py`,
  `webserver_async.py`, `wlan.py`). Config comes from `env.conf` on the device
  flash; see `env_example.conf`.
- **`gas-reed-rs/`** — work-in-progress Rust port using [embassy]. Scaffold only
  at this stage; the current goal is proving out toolchain and wifi connectivity
  via embassy's own examples. See `gas-reed-rs/README.md` for full setup.

Target hardware is a **Raspberry Pi Pico W (RP2040, 2022 revision)** —
`thumbv6m-none-eabi`, Cortex-M0+.

`gas-reed-rs/embassy/` is a plain clone of <https://github.com/embassy-rs/embassy>,
not a submodule, and is gitignored. Edits there are untracked and disposable.

## Build and flash

```sh
cd gas-reed-rs/embassy/examples/rp
cargo run --bin wifi_blinky --release
```

`.cargo/config.toml` in that directory already sets `target = "thumbv6m-none-eabi"`
and `runner = "probe-rs run --chip RP2040"`, so no extra flags are needed.

Use **`wifi_blinky`**, not `blinky`, on a Pico W. `blinky` drives GPIO25, which is
the LED on the plain Pico; on the W that pin goes to the CYW43439 and the LED
hangs off the wifi module. `wifi_blinky` also exercises the CYW43 firmware upload,
which everything else depends on.

## Environment gotchas

These have each cost real time. Check them before deeper debugging.

### `can't find crate for core` has two distinct causes

Always disambiguate first:

```sh
rustc --print target-libdir --target thumbv6m-none-eabi
```

- Prints `/usr/lib/rustlib/...` → **pacman's `rust` package is shadowing rustup.**
  `/usr/bin/rustc` precedes `~/.cargo/bin/rustc` in `PATH`, so the rustup shim
  never runs. The distro rustc has no bare-metal targets and `rustup target add`
  cannot give it any. Confirm with `type -a rustc`. Prepending `~/.cargo/bin` to
  `PATH` does **not** stick — mise's zsh `precmd` hook rewrites `PATH` every
  prompt. The fix is `sudo pacman -Rs rust` (safe: `Required By: None`, only an
  optional dep of `kate`).
- Prints a `~/.rustup/...` path but the build still fails → **the target is
  missing from the mise-selected toolchain.** Root `mise.toml` pins `rust = "1.97"`,
  which mise implements by exporting `RUSTUP_TOOLCHAIN=1.97.1`. That env var
  outranks `embassy/rust-toolchain.toml`, and rustup only auto-installs that
  file's `targets = [...]` when the file is what selects the toolchain. Fix with
  `rustup target add thumbv6m-none-eabi --toolchain 1.97.1-x86_64-unknown-linux-gnu`,
  or drop `rust` from `mise.toml` and let embassy's toolchain file govern.

A **full rebuild from scratch** (starting at `Compiling proc-macro2`) when
`target/` is already populated means the active rustc changed — a strong signal
for the first cause.

### Debug probe (official Raspberry Pi Debug Probe)

- Wiring: probe's **D** port (not `U`). Orange→SWCLK, black→GND, yellow→SWDIO,
  matched against the silkscreen labels on the Pico W's bottom edge.
- **The `D` port carries no power.** The Pico W needs its own 5V via its own USB
  cable. An unpowered target is the most common "not responding" cause.
- The probe must be plugged into the PC running `probe-rs`; the Pico W's cable
  can go to any power source.
- udev is already handled: `69-probe-rs.rules` ends with a
  `ATTRS{product}=="*CMSIS-DAP*"` catch-all, which the probe's
  `Debug Probe (CMSIS-DAP)` product string matches. No `2e8a` rule is needed.
- probe-rs 0.32 requires probe firmware **≥ 2.2.0**. 2022-era probes ship 1.01 and
  fail with "The firmware on the probe is outdated". Check with
  `lsusb -d 2e8a: -v 2>/dev/null | grep bcdDevice`; update by flashing
  `debugprobe.uf2` from [raspberrypi/debugprobe releases] via BOOTSEL. Take plain
  `debugprobe.uf2`, not `debugprobe_on_pico.uf2` (that's for a spare Pico used as
  a probe).

### Credentials

`wifi_tcp_server` and `wifi_webrequest` hardcode SSID and password as consts at
the top of the file. `wifi_blinky` and `wifi_scan` need none. When this moves into
`gas-reed-rs` proper, read them via `env!()` rather than hardcoding, mirroring how
`env.conf` works on the MicroPython side.

[embassy]: https://github.com/embassy-rs/embassy
[raspberrypi/debugprobe releases]: https://github.com/raspberrypi/debugprobe/releases

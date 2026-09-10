# Minimal setup — "hello world" (wifi_blinky) in `gas-reed-rs`

This documents the smallest set of files needed for `gas-reed-rs` to build and
flash a working blink on the Pico W, mirroring `embassy/examples/rp/src/bin/wifi_blinky.rs`.
Plain GPIO blinky (`Output::new(p.PIN_25, ...)`) does **not** work here — on the
Pico W, GPIO25 goes to the CYW43439 wifi chip, not the LED. The LED is only
reachable through the `cyw43` driver, so "hello world" on this board means
bringing up that chip.

## 1. Toolchain and target (once per machine)

```sh
rustup target add thumbv6m-none-eabi
cargo install probe-rs-tools
```

See the root `CLAUDE.md` for the two distinct `can't find crate for core`
failure modes if this doesn't just work.

## 2. Files required, and what each does

| File | Purpose |
| --- | --- |
| `Cargo.toml` | Dependency list (below) |
| `.cargo/config.toml` | Pins the build target and the flash/run command |
| `memory.x` | RP2040 flash/RAM layout for the linker |
| `build.rs` | Feeds `memory.x` to the linker, sets link args |
| `src/main.rs` | The actual program |
| `cyw43-firmware/*.bin` | Firmware blobs for the CYW43439, baked into the binary at compile time |

### `.cargo/config.toml`

```toml
[target.'cfg(all(target_arch = "arm", target_os = "none"))']
runner = "probe-rs run --chip RP2040"

[build]
target = "thumbv6m-none-eabi"

[env]
DEFMT_LOG = "debug"
```

Without this, `cargo build` defaults to your host target and dies with errors
like `invalid instruction mnemonic 'sev'` — that's ARMv6-M inline asm being fed
to the host (e.g. x86_64) assembler.

### `memory.x`

```
MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}
```

### `build.rs`

```rust
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
```

## 3. `Cargo.toml` dependencies

```toml
[package]
name = "gas-reed-rs"
version = "0.1.0"
edition = "2024"

[dependencies]
cortex-m = "0.7.9"
cortex-m-rt = "0.7.6"
critical-section = "1.2.0"
cyw43 = { version = "0.7.0", features = ["defmt", "firmware-logs"] }
cyw43-pio = { version = "0.10.0", features = ["defmt"] }
defmt = "1.1.1"
defmt-rtt = "1.3.0"
panic-probe = "1.0.0"
static_cell = "2.1.1"
embassy-executor = "0.10.0"
embassy-time = "0.5.1"
embassy-rp = { version = "0.10.0", features = [
    "critical-section-impl", "defmt", "executor-interrupt",
    "executor-thread", "rp2040", "time-driver", "unstable-pac",
] }
portable-atomic = { version = "1.5.0", features = ["critical-section"] }
```

`panic-probe` matters more than it looks: a `no_std` binary won't link at all
without exactly one `#[panic_handler]`, and this is what supplies it.
`static_cell` is what lets `cyw43::State` (large, must outlive the program) be
allocated without a heap. `portable-atomic` needs its `critical-section`
feature explicitly turned on — Cortex-M0+ (`thumbv6m`) has no native atomic
compare-exchange instruction, and without this feature `static_cell` fails to
build with `compare_exchange requires atomic CAS but not available on this
target by default`. The examples' `Cargo.toml` gets this for free because it
lists `portable-atomic` explicitly; a fresh `Cargo.toml` that never mentions it
pulls the crate in transitively without the feature enabled.

### API drift: local `embassy/` checkout vs. crates.io

`wifi_blinky.rs` in the embassy checkout is built (via `path = "../../cyw43"`
etc. in that example's own `Cargo.toml`) against the *local, in-development*
source, not what's actually published under the matching version number on
crates.io. Copying the example verbatim against plain crates.io versions (as
above, no `path =`) hits two API mismatches:

- `cyw43::Runner<'a, BUS, CHIP>` (local) vs. `cyw43::Runner<'a, BUS>` (published
  `0.7.0`) — no `CHIP` generic, no `Cyw43439` type, in the published version.
- `PioSpi::new(..., dma_tx, dma_rx)` (local, two DMA channels) vs.
  `PioSpi::new(..., dma)` (published `0.10.0`, one channel — the bus is
  half-duplex, one channel handles both directions).

Fix by matching the published API rather than adding a `path` dependency
(which would mean needing `embassy/` checked out forever just to build):

```rust
// cyw43_task's signature — no CHIP param:
runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,

// PioSpi::new — one DMA channel, not two:
let spi = PioSpi::new(
    &mut pio.common,
    pio.sm0,
    DEFAULT_CLOCK_DIVIDER,
    pio.irq0,
    cs,
    p.PIN_24,
    p.PIN_29,
    dma::Channel::new(p.DMA_CH0, Irqs),
);
```

`cyw43::new(...)`'s own signature (the firmware-loading call) is identical
between the local and published versions, so `fw`/`clm`/`nvram` need no
equivalent change — just the file paths (next section).

## 4. `src/main.rs`

```rust
#![no_std]
#![no_main]

use cyw43::aligned_bytes;
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, dma};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::main(executor = "embassy_rp::executor::Executor", entry = "cortex_m_rt::entry")]
async fn main(spawner: Spawner) {
    let fw = aligned_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = aligned_bytes!("../cyw43-firmware/43439A0_clm.bin");
    let nvram = aligned_bytes!("../cyw43-firmware/nvram_rp2040.bin");

    let p = embassy_rp::init(Default::default());

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        dma::Channel::new(p.DMA_CH0, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (_net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw, nvram).await;
    spawner.spawn(unwrap!(cyw43_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let delay = Duration::from_secs(1);
    loop {
        info!("led on!");
        control.gpio_set(0, true).await;
        Timer::after(delay).await;

        info!("led off!");
        control.gpio_set(0, false).await;
        Timer::after(delay).await;
    }
}
```

The firmware files live in `gas-reed-rs/cyw43-firmware/` — a local copy checked
into this project, not a reference into the `embassy/` clone — so the path
from `src/main.rs` is `../cyw43-firmware/...`. See the next section for why.

## 5. The firmware blobs — copy them locally, don't reference `embassy/`

`aligned_bytes!` reads a real file off disk at compile time; the path is
resolved relative to `src/main.rs`. Pointing it at `embassy/cyw43-firmware/...`
technically works, but it means `embassy/` (a multi-hundred-MB clone that's
gitignored and meant to be disposable, per the root `CLAUDE.md`) has to exist
on disk every time this project builds — the opposite of treating embassy as a
normal external dependency.

Instead, copy just the three files this board needs into the project itself:

```sh
mkdir -p cyw43-firmware
cp embassy/cyw43-firmware/43439A0.bin cyw43-firmware/
cp embassy/cyw43-firmware/43439A0_clm.bin cyw43-firmware/
cp embassy/cyw43-firmware/nvram_rp2040.bin cyw43-firmware/
```

These are small (~230KB total) vendor binary blobs — fine to commit. After
this, `embassy/` is purely a reference checkout for reading example source; the
build never touches it.

### Alternative: pull the firmware from a crate instead of a local copy

Two small **third-party, unofficial** crates package these same blobs as an
actual Cargo dependency, if you'd rather not hand-copy files at all:

- [`cyw43-firmware`](https://crates.io/crates/cyw43-firmware) (by KizzyCode) —
  raw blobs as `pub const` byte arrays (`CYW43_43439A0`, `CYW43_43439A0_CLM`, …).
- [`cyw43-setup`](https://crates.io/crates/cyw43-setup) (by jorgeandrecastro) —
  wraps the above and exposes `FW`, `CLM`, `NVRAM` as ready-to-use
  `Aligned<A4, [u8; N]>` constants, with `NVRAM` specifically bundling
  `nvram_rp2040.bin` — i.e. built for this exact board.

Neither is affiliated with the embassy project (the embassy org's own
`cyw43-firmware` package on crates.io has no library code — no `src/lib.rs` —
so it can't actually be depended on for this). Both third-party crates are
tiny, single-maintainer, and low-download-count — worth reading the source
before relying on them for anything beyond prototyping, which is why this
project sticks with a local copy for now. One thing not yet verified: whether
`cyw43::new()`'s expected `&Aligned<A4, [u8]>` (unsized slice) parameter
accepts `cyw43-setup`'s `Aligned<A4, [u8; N]>` (fixed-size array) constants
via reference coercion without extra glue — untested here.

## 6. Build and flash

With the debug probe attached and the Pico W powered:

```sh
cargo run --release
```

`.cargo/config.toml`'s `runner` handles flashing and streams `defmt` logs back
over RTT. Expect alternating `led on!` / `led off!` log lines and the LED
blinking at 1s/1s.

# Wifi provisioning via AP mode — feasibility notes (not implemented)

This is a design sketch for a "hold a button to reconfigure wifi" feature,
similar to how many consumer IoT devices let you set up wifi without hardcoding
credentials at build time: the device starts its own access point, you connect
to it and submit real credentials through a small web form, the device saves
them and reboots onto your actual network.

**Status: hypothetical.** Nothing below is implemented. This exists alongside
`.env`-based credentials (see `MINIMAL_SETUP.md`), which remain the simpler,
already-working path for a device that's installed once and rarely moves
networks. Read this as "here's what it would take," not a commitment to build
it.

## Is it feasible on this hardware?

Yes. The CYW43439 is a full 802.11 chip, not station-only, and the driver
already exposes it: `cyw43::Control` (the version already in this project's
`Cargo.toml`) has `start_ap_open(ssid, channel)` and
`start_ap_wpa2(ssid, passphrase, channel)` in `control.rs`. So bringing the
device up as its own access point needs no new dependency or driver work.

## Pieces required

### 1. A trigger: button held for N seconds

A GPIO button, read with `embassy_rp::gpio::Input`'s async edge-waiting
methods (`wait_for_falling_edge()`, `wait_for_high()`, etc. — same family used
for the reed sensor's edge detection). Detecting a *hold*, not just a press,
means racing two futures with `embassy_futures::select::select()`:

1. Wait for the press edge.
2. Race `input.wait_for_high()` (release) against
   `Timer::after(Duration::from_secs(N))`.
3. If the timer wins, the button was held past N seconds → trigger
   provisioning mode. If release wins first, it was an ordinary short press →
   ignore.

Runs as its own task, spawned alongside `cyw43_task`/`net_task`, following the
same one-task-per-concern structure as `main.py`'s `receive_value`/`heartbeat`.

### 2. Switching into AP mode: reboot, don't hot-swap

Tearing down station mode and bringing up AP mode live, in the same running
`Control`, is the harder path and not obviously supported cleanly. Simpler:
signal intent and soft-reset, so `main()` runs from scratch and decides which
mode to start in.

RP2040's watchdog peripheral has 8 scratch registers
(`embassy_rp::watchdog`'s `set_scratch(index, value)`) that **survive a soft
reset**. On hold-detected: write a marker into a scratch register, then
`cortex_m::peripheral::SCB::sys_reset()`. On the next boot, read that register
before deciding "join as station" vs. "call `start_ap_wpa2(...)` instead." No
flash write needed for this transient signal — flash stays reserved for the
actual saved credentials (next section).

### 3. A config web page, served from AP mode

Once in AP mode, serve a small HTML form over TCP via `embassy-net`, the same
hand-rolled request-parsing approach already used in `webserver_async.py`
(read request line, dispatch by method+path, write a raw HTTP response) —
just embassy-net's TCP sockets instead of MicroPython's.

Optional: a tiny DNS server that resolves every query to the device's own IP,
so phones/laptops auto-pop the "sign in to network" prompt the way commercial
captive portals do. Skippable — many devices just document "browse to
192.168.4.1."

### 4. Persistent storage — the actual gap

This is the piece that doesn't exist anywhere in this Rust port yet. The
MicroPython side gets a real filesystem for free (`persistence.py`,
`checkpoint.txt`); the Rust port has nothing analogous. Saving submitted
credentials means writing to RP2040's flash directly:

- `embassy_rp::flash`'s raw read/erase/write API, or
- `sequential-storage` (an embassy-ecosystem crate purpose-built for small,
  wear-leveled key-value data in flash) — probably the better fit, since flash
  has a limited erase-cycle count and naive repeated writes to the same region
  would wear it out over the device's lifetime.

On boot, if the watchdog scratch register doesn't request AP mode, read saved
credentials from flash instead of (or in addition to) `.env`-baked constants.

## Open questions / not yet resolved

- Whether `Control` truly can't switch AP↔station without a full re-init, or
  whether that was just assumed rather than confirmed by reading the driver's
  mode-switching code.
- Exact `sequential-storage` integration — how much flash to reserve, wear
  characteristics for this specific write frequency (essentially "once per
  reconfiguration," so wear is unlikely to matter much in practice).
- Whether the DNS-hijack captive-portal-popup piece is worth the extra code
  versus documenting a fixed IP.

## Why this might not be worth building

A gas meter reed sensor is a fixed installation — it goes in once, next to one
meter, and stays there. The realistic case for needing to change wifi
credentials at all is rare (router replacement, SSID rename). Given that,
`.env`-baked-at-build-time credentials (reflash to change them) may be the
right amount of engineering for this specific device, and the AP-mode flow
above is closer to "what commercial products with many non-technical end
users need" than "what this project needs." Worth weighing before committing
time to it.

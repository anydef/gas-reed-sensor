# gas-reed-rs

Work-in-progress Rust port of the MicroPython gas meter sensor, targeting the
Raspberry Pi Pico W (RP2040) with [embassy](https://github.com/embassy-rs/embassy).

Right now this is a scaffold. The immediate goal is proving out the toolchain and
the Pico W's wifi connectivity by running embassy's own examples.

For initial toolchain/probe setup and building/flashing embassy's examples, see
[EMBASSY_SETUP.md](EMBASSY_SETUP.md).

## Status

- [x] Toolchain builds for `thumbv6m-none-eabi`
- [x] Debug probe wired and detected by `probe-rs`
- [x] Probe firmware updated to ≥ 2.2.0
- [x] `wifi_blinky` verified on hardware
- [ ] `wifi_scan` / connectivity verified
- [ ] Reed sensor counting ported from `../reed.py` / `../meter_counter.py`
- [ ] Pushgateway client ported from `../pushgateway_client.py`
- [ ] Persistence ported from `../persistence.py`

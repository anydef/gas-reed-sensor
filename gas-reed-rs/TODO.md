# TODO: picoserve metrics endpoint + Prometheus collector

Goal: expose a `/metrics` HTTP endpoint from the Pico W using `picoserve`,
mirroring what `../webserver_async.py` already does on the MicroPython side.

## Reference

- Existing MicroPython webserver: `../webserver_async.py` — hand-rolled raw
  TCP parsing, routes: `GET /`, `GET /metrics`, `GET /checkpoint`,
  `GET /checkpoints`, `POST /meter`.
- Prometheus line format currently used (`webserver_async.py:23`):
  `gas_meter_counter_total {unit="0.01m3"} <value>`
- Raw `embassy_net::tcp::TcpSocket` accept-loop reference:
  `embassy/examples/rp/src/bin/wifi_tcp_server.rs:126-163`
- Reed contact → channel plumbing already wired in `src/main.rs`
  (`REED_CHANNEL`, `reed_task`, `led_task`) — the counter/collector needs to
  tap into this same event flow.

## Tasks

- [x] Add `picoserve` to `Cargo.toml`; check which feature flags it needs for
      `embassy-net`/`embedded-io-async` integration (no_std target).
- [x] Decide where the running total is stored. `REED_CHANNEL` is an event
      queue (`ReedState::Contact`), not state — need a separate shared
      counter a `/metrics` handler can read without consuming/blocking.
      Decided 2026-09-24: use the `embeprom` crate (no_std, no-heap Prometheus
      metrics for embedded, picoserve-friendly) instead of hand-rolling an
      `embassy_sync::Mutex<...>`. Its global accessor pattern
      (`metrics::get().foo.inc()`, callable from anywhere) replaces the need
      for manual cross-task synchronization between `reed_task` and
      `web_task` entirely.
- [ ] Spawn the picoserve TCP-accept loop as its own `#[embassy_executor::task]`,
      same shape as `cyw43_task`/`net_task`/`reed_task` in `src/main.rs`.
- [ ] Implement `GET /metrics` handler emitting the Prometheus text format
      line above, reading the shared counter.
- [ ] Decide scope for the other routes (`/checkpoint`, `/checkpoints`,
      `POST /meter`) — depends on whether `persistence.py`'s equivalent
      exists yet in the Rust port. Punt if not ported yet.
- [ ] Manual test: `curl http://<pico-ip>/metrics` after flashing, confirm
      output format is Prometheus-scrapeable (`promtool check metrics` or
      similar, if available).
- [ ] Reconcile with the CLAUDE.md project description ("pushes readings to
      a Prometheus Pushgateway") — confirm whether this pull-based `/metrics`
      endpoint is meant to replace, or sit alongside, the Pushgateway push
      path once that's ported too.

## Metric design: pulses counter + calibrated absolute reading

Decided 2026-09-24. Two metrics, not one, both driven off the same reed-pulse
event:

- **`gas_meter_pulses_total`** (Counter) — raw reed-contact count since last
  boot. Only ever incremented, never touched by calibration. Exists so
  `rate()`/`increase()` in Grafana/PromQL can graph consumption over time.
  Prometheus's counter-reset handling means this metric doesn't need to
  survive a reboot to stay useful for rate queries — resetting to 0 on
  restart is fine.
- **`gas_meter_reading`** (Gauge, not Counter) — the calibrated absolute
  value, matching the physical meter's dial. Incremented by the same pulses
  as the counter above during normal operation, but must also support an
  arbitrary overwrite (a `POST /meter`-equivalent calibration endpoint,
  mirroring `webserver_async.py`'s `set_meter_value` / `reed_counter.reset(new_val)`).
  Gauge is the correct Prometheus type here specifically because calibration
  is a `.set(value)` operation, not a monotonic increment/reset-to-zero —
  outside a Counter's contract.

**Blocking dependency:** unlike the pulses counter, this Gauge's value *must*
survive a device reboot to be trustworthy (that's its whole purpose — "what
does the meter say right now"). That means it needs to be seeded from
flash-persisted storage on boot, the same way `persistence.py`/
`checkpoint.txt`/`read_checkpoint()` do on the MicroPython side. No flash
persistence layer exists anywhere in this Rust port yet (same gap called out
in `WIFI_PROVISIONING.md`'s "Persistent storage" section — `embassy_rp::flash`
raw API or the `sequential-storage` crate are the two realistic options).
Sequencing: persistence needs solving before this Gauge is meaningful across
restarts, even though the metric itself could be stubbed in earlier (seeded
from `.env` or a hardcoded value) for initial testing.

- [ ] Add `embeprom` to `Cargo.toml`; declare the metrics group (`embeprom::metrics!`
      macro) with `gas_meter_pulses_total: Counter` and `gas_meter_reading: Gauge`.
- [ ] Wire both metrics to update together off the same reed-pulse event
      (currently only `ReedState::Contact` → `led_task`'s blink; no counter
      exists at all yet) — `reed_task` calls `metrics::get().gas_meter_pulses_total.inc()`
      and `metrics::get().gas_meter_reading.inc()` (via `Gauge::inc()`, confirmed
      present in `embeprom`'s API) on each contact.
- [ ] `GET /metrics` handler renders via `embeprom::Renderer`'s `next_line()`
      cursor rather than hand-formatting text.
- [ ] Add a calibration route (`POST /meter` equivalent) that calls
      `metrics::get().gas_meter_reading.set(new_val)` (`Gauge::set(&self, v: i64)`,
      confirmed present) — leaves the Counter untouched.
- [ ] Build/choose the flash persistence layer, then wire the Gauge's boot-time
      seed and the calibration route's write-through to it.
- [ ] Reserve a fixed flash region for `sequential-storage` before wiring it
      up: shrink `memory.x`'s `FLASH` `LENGTH` (currently `2048K - 0x100`, i.e.
      the whole chip) so the linker never places code/data in the reserved
      tail, then point `sequential-storage`'s config range at that same
      excluded region. These two numbers only agree because a human keeps
      them in sync — nothing checks this at build or runtime, so a mismatch
      here is a silent flash-corruption risk, not a compile error.
      Current baseline (`size target/thumbv6m-none-eabi/release/gas-reed-rs`,
      2026-09-24): `text` 484904 + `data` 56 ≈ 474KB of 2048KB used (`bss`
      51012 is RAM-only, not flash). Reserving the last 64KB leaves large
      headroom — recheck `size` after adding the persistence/metrics code
      before finalizing the boundary, since this baseline predates that work.

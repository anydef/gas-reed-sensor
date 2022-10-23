import uasyncio as asyncio
from machine import Pin
from uasyncio import Event

import boot
import meter_counter
import persistence
import pushgateway_client
import webserver_async as webserver
from reed import AsyncPin, Meter, do_on_event
from utils import Semaphore

IRQ_BACKOFF_MS = 3000

print("Booting up.")

onboard_led = Pin("LED", Pin.OUT)
onboard_led.off()

meter_led = Pin(15, Pin.OUT)
meter_led.off()

reed_pin = AsyncPin(Pin(14, Pin.IN, Pin.PULL_DOWN),
                    Pin.IRQ_RISING, callback_backoff=IRQ_BACKOFF_MS)

checkpoint = persistence.read_checkpoint()

reed_event = Event()
reed_counter_total = Meter(checkpoint)
reed_current_reading = Meter()
heartbeat_semaphore = Semaphore()


async def receive_value(async_pin, event):
    while True:
        await async_pin.wait_edge()
        if async_pin.value() == 1:
            event.set()
            await reed_counter_total.inc()
            await reed_current_reading.inc()


async def send_readings(delay):
    while True:
        await asyncio.sleep(delay)
        pushgateway_client.send_gas_meter_total(reed_counter_total.value)

        # Reset current reading since pushgateway doesn't have ttl
        pushgateway_client.send_gas_meter_reading(reed_current_reading.value)
        await reed_current_reading.reset()


async def heartbeat(led, delay):
    while True:
        led.on()
        await asyncio.sleep(0.25)
        led.off()
        await asyncio.sleep(delay)


async def heartbeat_metric(delay):
    while True:
        await asyncio.sleep(delay)

        pushgateway_client.send_heartbeat(
            1 if heartbeat_semaphore.toggle() else 0)


async def flush_meter(meter, delay):
    while True:
        persistence.persist_meter_status(meter.value)
        persistence.persist_checkpoint(meter.value)
        await asyncio.sleep(delay)


async def main():
    boot.init()
    # start webserver
    asyncio.create_task(asyncio.start_server(
        webserver.create_server(reed_counter=reed_counter_total), "0.0.0.0", 80))

    # listen to reed pin irq events
    asyncio.create_task(receive_value(reed_pin, reed_event))

    asyncio.create_task(do_on_event(reed_event,
                                    (meter_counter.blink, meter_led),
                                    )
                        )
    # send heartbeat metric every minute.
    # this is required since gas meter might be idling,
    # and monitoring system will not be able to recognize whether the module is running.
    asyncio.create_task(heartbeat_metric(60))
    asyncio.create_task(send_readings(60))
    # flush meter status every hour into the file.
    asyncio.create_task(flush_meter(reed_counter_total, 3600))

    # onboard led heartbeat. this loop keeps event loop intact.
    await heartbeat(onboard_led, 5)


try:
    asyncio.run(main())
finally:
    asyncio.new_event_loop()

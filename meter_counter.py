import uasyncio as asyncio


async def blink(led):
    led.on()
    await asyncio.sleep(0.2)
    led.off()

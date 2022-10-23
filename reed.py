import uasyncio as asyncio
import utime as time


class Meter(object):
    def __init__(self, init_value=0):
        self.__counter = init_value
        self.mutex = asyncio.Lock()

    async def inc(self, count=1):
        await self.mutex.acquire()
        self.__counter = self.__counter + count
        self.mutex.release()

    async def reset(self, new_value=0):
        await self.mutex.acquire()
        self.__counter = new_value
        self.mutex.release()

    @property
    def value(self):
        return self.__counter


class AsyncPin:

    def __init__(self, pin, trigger, callback_backoff=0):
        self.callback_backoff = callback_backoff
        self.last_tick = 0
        self.pin = pin
        self.flag = asyncio.ThreadSafeFlag()
        self.pin.irq(lambda pin: self.callback(pin), trigger, hard=False)

    def callback(self, pin):
        now = time.ticks_ms()
        if time.ticks_diff(now, self.last_tick) > self.callback_backoff:
            self.flag.set()

        self.last_tick = now

    def value(self):
        return self.pin.value()

    async def wait_edge(self):
        await self.flag.wait()


async def do_on_event(event, *tasks):
    while True:
        await event.wait()
        for t in tasks:
            await t[0](*t[1:])
        event.clear()

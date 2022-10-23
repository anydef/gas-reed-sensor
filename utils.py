import uasyncio as asyncio


class Semaphore(object):
    def __init__(self):
        self.__status = False
        self.mutex = asyncio.Lock()

    async def toggle(self):
        await self.mutex.acquire()
        self.__status = not self.__status
        self.mutex.release()
        return self.value

    async def reset(self, new_val=False):
        await self.mutex.acquire()
        self.__status = new_val
        self.mutex.release()

    @property
    def value(self):
        return self.__status

import time

import ntptime

max_retries = 10


def __set_time(no):
    if no > max_retries: return
    try:
        ntptime.settime()
    except OSError as ex:
        print(ex)
        __set_time(no + 1)


def init():
    __set_time(0)


def current_time_ms():
    return time.time()


def local_time():
    t = time.localtime()
    return f"{t[0]}-{t[1]:02d}-{t[2]:02d} {t[3]:02d}:{t[4]:02d}"

from time import sleep

import network
import ubinascii
from machine import Pin

import config
import pico_time


def init():
    wlan = network.WLAN(network.STA_IF)
    if wlan.isconnected():
        wlan.disconnect()

    led = Pin("LED", Pin.OUT)
    cfg = config.read_config()
    ssid = cfg['wifi_ssid']
    password = cfg['wifi_secret']
    led.on()

    # Connect to WLAN
    wlan.active(True)
    wlan.connect(ssid, password)

    while not wlan.isconnected():
        led.on()
        sleep(0.25)
        led.off()
        sleep(0.25)

    print(wlan.ifconfig())

    mac = ubinascii.hexlify(wlan.config('mac'), ':').decode()
    led.off()
    print(mac)
    pico_time.init()
    print(f"Booted at {pico_time.local_time()}")

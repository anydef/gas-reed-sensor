from time import sleep

import machine
import network

import config


def connect():
    cfg = config.read_config()
    wlan = network.WLAN(network.STA_IF)
    wlan.active(True)
    wlan.connect(cfg['wifi_ssid'], cfg['wifi_secret'])
    while not wlan.isconnected():
        print('Waiting for connection...')
        sleep(1)
    print(wlan.ifconfig())


try:
    connect()
except KeyboardInterrupt:
    machine.reset()

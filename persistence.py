import pico_time


def persist_meter_status(value):
    with open('persistence.txt', 'a') as f:
        f.write(f'{value},{pico_time.current_time_ms()},{pico_time.local_time()}\n')


def checkpoints():
    with open('persistence.txt', 'r') as f:
        return f.readlines()


def persist_checkpoint(value):
    with open('checkpoint.txt', 'w') as f:
        f.write(f'{value},{pico_time.current_time_ms()},{pico_time.local_time()}\n')


def last_checkpoint():
    with open('checkpoint.txt', 'r') as f:
        return f.readline()


def read_checkpoint() -> int:
    with open('checkpoint.txt', 'r') as f:
        checkpoint = f.readline()
    if checkpoint:
        return int(checkpoint.split(",")[0])
    return 0

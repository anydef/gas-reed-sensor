import socket

import config

cfg = config.read_config()
host = cfg['pushgateway_host']
port = int(cfg['pushgateway_port'])
job = "gas-meter"
instance = cfg['instance']


def socket_send(sock, metric_name, value, metric_type='COUNTER'):
    """All whitespaces and linebreaks are according to HTTP/1.1 specification. Must be preserved."""
    data = """#TYPE {metric_name} {metric_type}
{metric_name} {value}\n""".format(metric_name=metric_name, metric_type=metric_type, value=value)
    request = """POST /metrics/job/{job}/instance/{instance} HTTP/1.1
Host: {host}:{port}
User-Agent: raspberry-pico
Accept: */*
Content-Type: application/x-www-form-urlencoded
Content-Length: {content_length}

""".format(
        content_length=str(len(data)),
        job=job,
        instance=instance,
        host=host,
        port=port
    )
    sock.send(request.encode("utf-8"))
    sock.send(data.encode("utf-8"))
    _ = sock.recv(512)


def send(metric_name, metric_value, metric_type):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sockaddr = socket.getaddrinfo(host, port)[0][-1]
    try:
        sock.connect(sockaddr)
        socket_send(sock, metric_name, metric_value, metric_type)
    except Exception as ex:
        print(ex)
    finally:
        sock.close()


def send_gas_meter_reading(value):
    send('gas_meter_reading', value, 'GAUGE')


def send_gas_meter_total(value):
    send('gas_meter_counter_total', value, 'COUNTER')


def send_heartbeat(val=1):
    send('gas_meter_heartbeat', val, 'GAUGE')

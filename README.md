# What is this?
It is a small project that allows flushing Gas meter sensors (reed-base) data into the Pushgateway.

# Why push gateway?
Atm. I am not interested in mqtt solution and am piggybacking on the already running Grafana+Prometheus+Pushgateway setup.

# Known issues.
For unknown to me reasons, reed-sensor either gets duplicate readings or gets "stuck" for a brief moment.
I found a sweet spot of 3000ms backoff (I'm not burning gas with such intensity anyways[10L in 3000ms]). 
This doesn't mean that sensor data is 100% accurate, but is enough to make an educated decision based on data.
Measurable deviation is about 1% over 3 months of readings, though it might vary from one sensor/meter to the other.


# Installation
Connect and flush Pi Pico-W with the latest Micropython installation. 
At the moment of writing 1.19.1 was used.


## Configuration
Create `env.conf` file and copy to the root of the device flash drive. 
Include network settings and target pushgateway.   
Example at `env_example.conf`.

## Tools
Pi Pico W. Thonny IDE or VSCode + Pico-W plugin. Micropython (atm. 1.19.1)


## API



### Set current meter reading


```
POST http://192.168.178.68/meter
Accept: application/json

12345

###
where 12345 is a current reading 
```

### Get current reading on the device

```
GET http://192.168.178.68/metrics
```

### Get all checkpoints

```
GET http://192.168.178.68/checkpoints
```


### Get last checkpoint

```
GET http://192.168.178.68/checkpoint
```



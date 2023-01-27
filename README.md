# Installation
Connect and flush Pi Pico-W with the latest Micropython installation. 
At the moment of writing 1.19.1 was used.


## Configuration
Create `env.conf` file and copy to the root of the device flash drive. 
Include network settings and target pushgateway.   
You can peek the example at `env_example.conf`.

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

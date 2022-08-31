# Apache SkyWalking PHP Agent - Config

## Add SkyWalking config to php.ini

```shell
extension = skywalking_agent

# Enable skywalking extension or not.
skywalking_agent.enable = On

# Log file path.
skywalking_agent.log_file = /tmp/skywalking_agent.log

# Log level: one of `OFF`, `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`.
skywalking_agent.log_level = INFO

# Address of skywalking oap server.
skywalking_agent.server_addr = http://0.0.0.0:11800

# Application service name.
skywalking_agent.service_name = hello-skywalking

# Skywalking version.
skywalking_agent.skywalking_version = 8

# Skywalking worker threads, 0 will auto set as the cpu core size.
# skywalking_agent.worker_threads = 3
```

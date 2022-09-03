# Setup PHP Agent

1. Agent is available for PHP 7.0 - 8.x
2. Build from source
3. Configure php.ini

## Build from source

```shell script
sudo apt install gcc make rust libclang
```

```shell script
git clone --recursive https://github.com/apache/skywalking-php.git

cd skywalking-php

# Optional, specify if php isn't installed globally.
# export PHP_CONFIG=<Your path of php-config>

# Build libskywalking_agent.so.
make build

# Install to php extension dir.
sudo make install
```

## Configure php.ini
```ini
[skywalking]
extension=skywalking_agent.so

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
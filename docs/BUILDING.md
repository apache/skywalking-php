# Apache SkyWalking PHP Agent - Building from source

This document has detailed instructions on how to build Apache SkyWalking PHP Agent from source.

## Pre-requisites

1. gcc
2. make
3. rust
4. libclang

## Build from source (PHP Extension)

```shell script
git clone --recursive https://github.com/apache/skywalking-php.git

cd skywalking-php

# Optional, specify if php isn't installed globally.
# export PHP_CONFIG=<Your path of php-config>

# Build libskywalking_agent.so.
make build

# Install to php extension dir.
make install
```

# Setup PHP Agent

1. Agent is available for PHP 7.2 - 8.x.
2. Build from source.
3. Configure `php.ini`.

## Requirements

- GCC
- Rustc 1.56+
- Cargo
- Libclang 9.0+
- Make
- Protoc

For Debian-base OS:

```shell
sudo apt install gcc make llvm-dev libclang-dev clang protobuf-compiler
```

### Install Rust globally

*Refer to <https://forge.rust-lang.org/infra/other-installation-methods.html#standalone-installers>.*

For linux x86_64 user:

```shell
wget https://static.rust-lang.org/dist/rust-1.65.0-x86_64-unknown-linux-gnu.tar.gz
tar zxvf rust-1.65.0-x86_64-unknown-linux-gnu.tar.gz
cd rust-1.65.0-x86_64-unknown-linux-gnu
./install.sh
```

## Install

### Install from pecl.net

```shell script
pecl install skywalking_agent
```

### install from the source codes

```shell script
git clone --recursive https://github.com/apache/skywalking-php.git
cd skywalking-php

phpize
./configure
make
make install
```

## Configure php.ini

```ini
[skywalking_agent]
extension=skywalking_agent.so

; Enable skywalking_agent extension or not.
skywalking_agent.enable = On

; Log file path.
skywalking_agent.log_file = /tmp/skywalking-agent.log

; Log level: one of `OFF`, `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`.
skywalking_agent.log_level = INFO

; Address of skywalking oap server.
skywalking_agent.server_addr = http://0.0.0.0:11800

; Application service name.
skywalking_agent.service_name = hello-skywalking

; Skywalking version.
skywalking_agent.skywalking_version = 8

; Skywalking worker threads, 0 will auto set as the cpu core size,
; default is 0.
; skywalking_agent.worker_threads = 3

; Skywalking agent runtime directory, default is /tmp/skywalking-agent.
; skywalking_agent.runtime_dir = /tmp/skywalking-agent
```

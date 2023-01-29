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
skywalking_agent.server_addr = 127.0.0.1:11800

; Application service name.
skywalking_agent.service_name = hello-skywalking

; Skywalking version.
skywalking_agent.skywalking_version = 8

; Skywalking authentication token, let it empty if the backend isn't enabled.
; skywalking_agent.authentication =

; Skywalking worker threads, 0 will auto set as the cpu core size, default is 0.
; skywalking_agent.worker_threads = 3

; Skywalking agent runtime directory, default is /tmp/skywalking-agent.
; skywalking_agent.runtime_dir = /tmp/skywalking-agent

; Wether to enable tls for gPRC, default is false.
; skywalking_agent.enable_tls = Off

; The gRPC SSL trusted ca file.
; skywalking_agent.ssl_trusted_ca_path =

; The private key file. Enable mTLS when ssl_key_path and ssl_cert_chain_path exist.
; skywalking_agent.ssl_key_path =

; The certificate file. Enable mTLS when ssl_key_path and ssl_cert_chain_path exist.
; skywalking_agent.ssl_cert_chain_path =

; Agent heartbeat report period. Unit, second. Default is 30.
; skywalking_agent.heartbeat_period = 30

; The agent sends the instance properties to the backend every
; heartbeat_period * properties_report_period_factor seconds. Default is 10.
; skywalking_agent.properties_report_period_factor = 10
```

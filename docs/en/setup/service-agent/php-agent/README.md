# Setup PHP Agent

1. Agent is available for PHP 7.2 - 8.x.
2. Build from source.
3. Configure `php.ini`.

## Requirements

- GCC / Clang
- Rustc 1.85+
- Cargo
- Libclang 9.0+
- Make
- Protoc

## Install dependencies

### For Debian-base OS

```shell
sudo apt install gcc make llvm-13-dev libclang-13-dev protobuf-c-compiler protobuf-compiler
```

### For Alpine Linux

```shell
apk add gcc make musl-dev llvm15-dev clang15-dev protobuf-c-compiler
```

## Install Rust globally

The officially recommended way to install Rust is via [`rustup`](https://www.rust-lang.org/tools/install).

> **Notice:** Because the source code toolchain is override by `rust-toolchain.toml`,
> so if you don't need multi version Rust, we recommend to install Rust by these
> way:
> 
> 1. Install through OS package manager (The Rust version in the source must be >= 1.85).
> 
> 2. Through `rustup` but set `default-toolchain` to none.
> 
>    ```shell
>    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none
>    ```

## Install

> **Notice:** If you compile `skywalking_agent` in Alpine Linux, you have to disable `crt-static`,
> otherwise the problem will be throw: "the libclang shared library at
> /usr/lib/libclang.so.15.0.7 could not be opened: Dynamic loading not supported".
>
> You can disable `crt-static` by environment variable:
>
> ```shell
> export RUSTFLAGS="-C target-feature=-crt-static"
> ```

### Install from pecl.net

```shell script
pecl install skywalking_agent
```

### Install from the source codes

```shell script
git clone --recursive https://github.com/apache/skywalking-php.git
cd skywalking-php

phpize
./configure
make
make install
```

## Configure

For scenarios where php-fpm runs in the foreground (`php-fpm -F`), or where a PHP script starts
a single Swoole server, you can use the `grpc` reporter mode.

For scenarios where php-fpm runs as a daemon, or where a PHP script forks multiple Swoole servers,
it is recommended to use the `standalone` reporter mode.

Configure skywalking agent in your `php.ini`.

```ini
[skywalking_agent]
extension = skywalking_agent.so

; Enable skywalking_agent extension or not.
skywalking_agent.enable = Off

; Reporter type, optional values are `grpc`, `kafka` and `standalone`.
skywalking_agent.reporter_type = grpc

; Log file path.
skywalking_agent.log_file = /tmp/skywalking-agent.log

; Log level: one of `OFF`, `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`.
skywalking_agent.log_level = INFO

; Address of skywalking oap server.
skywalking_agent.server_addr = 127.0.0.1:11800

; Application service name.
skywalking_agent.service_name = hello-skywalking
```

Refer to the Configuration section for more configuration items.

> **Notice:** It is not recommended to enable `skywalking_agent.enable` by default globally,
> because skywalking agent will modify the hook function and fork a new process to be a worker.
> Enabling it by default will cause extra meaningless consumption when skywalking agent is not
> needed (such as simply executing a php script).

### PHP Health Metrics (PHM)

> **Platform:** PHM process meters are **Linux only**. The forked reporter worker reads the
> parent PHP process via `/proc` (`/proc/{pid}/status`, `stat`, and `fd`). They are not available
> on macOS or Windows. Trace and other agent features are unchanged.

When `reporter_type` is `grpc` or `kafka`, the forked reporter worker boots
`skywalking::metrics::Metricer` in `start_worker`, alongside heartbeat reporting. A background
collector samples `/proc` for the parent PHP process (`getppid()`), updates Gauges, and `Metricer`
reports meter data to OAP through the same path as traces and logs. PHM does not run when
`reporter_type = standalone`.

PHM reports PHP runtime meters through the native Meter protocol (MeterReportService), without
requiring HTTP traffic, similar to Python PVM and Ruby runtime meters.
**PHM is enabled by default on Linux** when the agent is active (`skywalking_agent.enable = On`).
To disable it or tune the interval, use `php.ini`:

```ini
; Disable PHM if not needed (default is On on Linux).
; skywalking_agent.metrics_enable = Off

; Report interval in seconds (default 30).
skywalking_agent.metrics_report_period = 30
```

PHM reports six process meters (aligned with OAP `php-runtime.yaml` and Horizon UI widgets):

| Agent meter name | OAP / UI expression | Source |
| --- | --- | --- |
| `instance_php_process_cpu_utilization` | `meter_instance_php_process_cpu_utilization` | `/proc/{pid}/stat` utime+stime delta |
| `instance_php_memory_used_mb` | `meter_instance_php_memory_used_mb` | `/proc/{pid}/status` VmRSS |
| `instance_php_memory_peak_mb` | `meter_instance_php_memory_peak_mb` | `/proc/{pid}/status` VmHWM |
| `instance_php_virtual_memory_mb` | `meter_instance_php_virtual_memory_mb` | `/proc/{pid}/status` VmSize |
| `instance_php_thread_count` | `meter_instance_php_thread_count` | `/proc/{pid}/status` Threads |
| `instance_php_open_fd_count` | `meter_instance_php_open_fd_count` | `/proc/{pid}/fd` count |

On the OAP side, activate the `php-runtime` entry in
`agent-analyzer.default.meterAnalyzerActiveFiles`. Horizon UI shows the widgets on the **General
Service → Instance** dashboard when data is available.

See [INI Settings](../../../configuration/ini-settings.md) for all PHM options.

## Run

Start `php-fpm` server:

```shell
php-fpm -F -d "skywalking_agent.enable=On"
```

> **Notice:** It is necessary to keep the `php-fpm` process running in the foreground
> (by specifying the `-F` parameter, etc.), or switch to using the `standalone` reporter mode.
> Running `php-fpm` as a daemon in `grpc` reporter mode will cause the `skywalking-agent` reporter
> process immediately exit.

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

## Install dependencies

### For Debian-base OS

```shell
sudo apt install gcc make llvm-13-dev libclang-13-dev protobuf-c-compiler
```

### For Alpine Linux

```shell
apk add gcc make musl-dev llvm15-dev clang15-dev protobuf-c-compiler
```

## Install Rust globally

The officially recommended way to install Rust is via [`rustup`](https://www.rust-lang.org/tools/install).

But because the source code toolchain is override by `rust-toolchain.toml`,
so if you don't need multi version Rust, we recommend to install Rust by these
way:

1. Install through OS package manager (The Rust version in the source must be >= 1.65).

2. Through [standalone installers](https://forge.rust-lang.org/infra/other-installation-methods.html#standalone-installers).

   For linux x86_64 user:

   ```shell
   wget https://static.rust-lang.org/dist/rust-1.65.0-x86_64-unknown-linux-gnu.tar.gz
   tar zxvf rust-1.65.0-x86_64-unknown-linux-gnu.tar.gz
   cd rust-1.65.0-x86_64-unknown-linux-gnu
   ./install.sh
   ```

3. Through `rustup` but set `default-toolchain` to none.

   ```shell
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none
   ```

## Install

> If you compile `skywalking_agent` in Alpine Linux, you have to disable `crt-static`, otherwise
> the problem will be throw: "the libclang shared library at /usr/lib/libclang.so.15.0.7 could not
> be opened: Dynamic loading not supported".
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

Configure skywalking agent in your `php.ini`.

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
```

Refer to the Configuration section for more configuration items.

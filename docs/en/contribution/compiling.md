# Compiling project

This document will help you compile and build the package file.

Prepare PHP and Rust environments.

## Install PHP Environment

For Debian user:

```shell
sudo apt install php-cli php-dev
```

For MacOS user:

```shell
brew install php
```

## Install Rust Environment

Install Rust globally.

For Linux user:

```shell
curl https://sh.rustup.rs -sSf | sudo sh -s -- --no-modify-path
sudo ln -s /root/.cargo/bin/rustup /usr/local/bin/rustup
sudo ln -s /root/.cargo/bin/cargo /usr/local/bin/cargo
```

For MacOS user:

```shell
brew install rust
```

## Install requirement

For Debian user:

```shell
sudo apt install gcc make libclang protobuf-compiler
```

For MacOS user:

```shell
brew install protobuf
```

## Build and install Skywalking PHP Agent from archive file

For Linux user:

```shell
sudo pecl install skywalking_agent-x.y.z.tgz
```

For MacOS user:

> Running the `pecl install` command with the php installed in brew may encounter the problem of `mkdir`, please refer to
> [Installing PHP and PECL Extensions on MacOS](https://patriqueouimet.ca/tip/installing-php-and-pecl-extensions-on-macos).

```shell
pecl install skywalking_agent-x.y.z.tgz
```

The extension file `skywalking_agent.so` is generated in the php extension folder, get it by run `php-config --extension-dir`.

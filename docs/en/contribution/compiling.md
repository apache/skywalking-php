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

```shell
export RUSTUP_HOME=/opt/rustup
export CARGO_HOME=/opt/cargo

curl https://sh.rustup.rs -sSf | sudo -E sh -s -- --no-modify-path
sudo ln -s $CARGO_HOME/bin/rustup /usr/local/bin/rustup
sudo ln -s $CARGO_HOME/bin/rustc /usr/local/bin/rustc
sudo ln -s $CARGO_HOME/bin/cargo /usr/local/bin/cargo
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

## Build and install Skywalking PHP Agent

* If you clone codes from https://github.com/apache/skywalking-php

   ```shell
   git clone --recursive https://github.com/apache/skywalking-php.git
   cd skywalking-php
   
   phpize
   ./configure
   make
   sudo make install
   ```

* If you download package tar from https://skywalking.apache.org/downloads/

   ```shell
   sudo pecl install skywalking_agent-x.y.z.tgz
   ```

The extension file `skywalking_agent.so` is generated in the php extension folder, get it by run `php-config --extension-dir`.

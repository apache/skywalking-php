# Licensed to the Apache Software Foundation (ASF) under one or more
# contributor license agreements.  See the NOTICE file distributed with
# this work for additional information regarding copyright ownership.
# The ASF licenses this file to You under the Apache License, Version 2.0
# (the "License"); you may not use this file except in compliance with
# the License.  You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

FROM php:8.1-fpm-bullseye as builder

ARG SKYWALKING_AGENT

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.64.0


RUN apt update \
        && apt install -y wget protobuf-compiler libclang-dev \
        && wget https://static.rust-lang.org/rustup/archive/1.25.1/x86_64-unknown-linux-gnu/rustup-init \
        && chmod +x rustup-init \
        && ./rustup-init -y --no-modify-path --profile minimal --default-toolchain $RUST_VERSION --default-host x86_64-unknown-linux-gnu \
        && rm rustup-init \
        && chmod -R a+w $RUSTUP_HOME $CARGO_HOME \
        && pecl install skywalking_agent-$SKYWALKING_AGENT \
        && docker-php-ext-enable skywalking_agent \
        && apt-get remove -y --auto-remove wget protobuf-compiler \
        && rm -rf /var/lib/apt/lists/*

FROM php:8.1-fpm-bullseye
LABEL org.opencontainers.image.source=https://github.com/apache/skywalking-php
LABEL org.opencontainers.image.description="The PHP Agent for Apache SkyWalking, which provides the native tracing abilities for PHP project."
LABEL org.opencontainers.image.licenses="Apache 2.0"
COPY --from=builder /usr/local/etc/php/conf.d/docker-php-ext-skywalking_agent.ini /usr/local/etc/php/conf.d/
COPY --from=builder /usr/local/lib/php/extensions/no-debug-non-zts-20210902/skywalking_agent.so /usr/local/lib/php/extensions/no-debug-non-zts-20210902/
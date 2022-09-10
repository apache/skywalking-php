dnl Licensed to the Apache Software Foundation (ASF) under one or more
dnl contributor license agreements.  See the NOTICE file distributed with
dnl this work for additional information regarding copyright ownership.
dnl The ASF licenses this file to You under the Apache License, Version 2.0
dnl (the "License"); you may not use this file except in compliance with
dnl the License.  You may obtain a copy of the License at
dnl
dnl     http://www.apache.org/licenses/LICENSE-2.0
dnl
dnl Unless required by applicable law or agreed to in writing, software
dnl distributed under the License is distributed on an "AS IS" BASIS,
dnl WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
dnl See the License for the specific language governing permissions and
dnl limitations under the License.

PHP_ARG_ENABLE([skywalking_agent],
  [whether to enable skywalking_agent support],
  [AS_HELP_STRING([--enable-skywalking_agent],
    [Enable skywalking_agent support])],
  [no])

PHP_ARG_ENABLE([cargo_debug], [whether to enable cargo debug mode],
[  --enable-cargo-debug           Enable cargo debug], no, no)

if test "$PHP_THREAD_SAFETY" == "yes"; then
  AC_MSG_ERROR([skywalking_agent does not support ZTS])
fi

if test "$PHP_SKYWALKING_AGENT" != "no"; then
  AC_PATH_PROG(CARGO, cargo, no)
  if ! test -x "$CARGO"; then
    AC_MSG_ERROR([cargo command missing, please reinstall the cargo distribution])
  fi

  AC_PATH_PROG(PROTOC, protoc, no)
  if ! test -x "$PROTOC"; then
    AC_MSG_ERROR([protoc command missing, please reinstall the protoc distribution])
  fi

  AC_DEFINE(HAVE_SKYWALKING_AGENT, 1, [ Have skywalking_agent support ])

  PHP_NEW_EXTENSION(skywalking_agent, [ ], $ext_shared)

  CARGO_MODE_FLAGS="--release"
  CARGO_MODE_DIR="release"

  if test "$PHP_CARGO_DEBUG" != "no"; then
    CARGO_MODE_FLAGS=""
    CARGO_MODE_DIR="debug"
  fi

  echo -e "./modules/skywalking_agent.so:\n\
\tPHP_CONFIG=$PHP_PHP_CONFIG cargo build $CARGO_MODE_FLAGS\n\
\tif [[ -f ./target/$CARGO_MODE_DIR/libskywalking_agent.dylib ]] ; then \
cp ./target/$CARGO_MODE_DIR/libskywalking_agent.dylib ./modules/skywalking_agent.so ; fi\n\
\tif [[ -f ./target/$CARGO_MODE_DIR/libskywalking_agent.so ]] ; then \
cp ./target/$CARGO_MODE_DIR/libskywalking_agent.so ./modules/skywalking_agent.so ; fi\n\
" > Makefile.objects

  PHP_MODULES="./modules/skywalking_agent.so"

  AC_CONFIG_LINKS([ \
    .rustfmt.toml:.rustfmt.toml \
    Cargo.lock:Cargo.lock \
    Cargo.toml:Cargo.toml \
    LICENSE:LICENSE \
    NOTICE:NOTICE \
    README.md:README.md \
    build.rs:build.rs \
    docker-compose.yml:docker-compose.yml \
    docs:docs \
    scripts:scripts \
    src:src \
    tests:tests \
    ])
fi

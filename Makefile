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

IS_DEBUG ?= 0
CARGO_BIN ?= cargo
CARGO_BUILD_FLAGS ?= 
CARGO_TEST_FLAGS ?= 
COMPOSER_BIN ?= composer

cargo_flags != if [ $(IS_DEBUG) = 1 ]; then echo "" ; else echo "--release" ; fi
target_dir != if [ $(IS_DEBUG) = 1 ]; then echo "debug" ; else echo "release" ; fi


all: build

build:
	$(CARGO_BIN) build $(cargo_flags) $(CARGO_BUILD_FLAGS)

test: build composer-install
	$(CARGO_BIN) test $(cargo_flags) $(CARGO_TEST_FLAGS)

install: build
	./target/$(target_dir)/skywalking install

composer-install:
	$(COMPOSER_BIN) install --working-dir=tests/php

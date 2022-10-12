// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![warn(rust_2018_idioms, missing_docs)]
#![warn(clippy::dbg_macro, clippy::print_stdout)]
// #![doc = include_str!("../README.md")]

mod channel;
mod component;
mod context;
mod execute;
mod module;
mod plugin;
mod request;
mod util;
mod worker;

use phper::{
    ini::{Ini, Policy},
    modules::Module,
    php_get_module,
};

/// Enable agent and report or not.
const SKYWALKING_AGENT_ENABLE: &str = "skywalking_agent.enable";

/// Version of skywalking server.
const SKYWALKING_AGENT_SKYWALKING_VERSION: &str = "skywalking_agent.skywalking_version";

/// skywalking server address.
const SKYWALKING_AGENT_SERVER_ADDR: &str = "skywalking_agent.server_addr";

/// skywalking app service name.
const SKYWALKING_AGENT_SERVICE_NAME: &str = "skywalking_agent.service_name";

/// Tokio runtime worker threads.
const SKYWALKING_AGENT_WORKER_THREADS: &str = "skywalking_agent.worker_threads";

/// Log level of skywalking agent.
const SKYWALKING_AGENT_LOG_LEVEL: &str = "skywalking_agent.log_level";

/// Log file of skywalking agent.
const SKYWALKING_AGENT_LOG_FILE: &str = "skywalking_agent.log_file";

#[php_get_module]
pub fn get_module() -> Module {
    let mut module = Module::new(
        env!("CARGO_CRATE_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS"),
    );

    // Register skywalking ini.
    Ini::add(SKYWALKING_AGENT_ENABLE, false, Policy::System);
    Ini::add(SKYWALKING_AGENT_SKYWALKING_VERSION, 8i64, Policy::System);
    Ini::add(
        SKYWALKING_AGENT_SERVER_ADDR,
        "http://127.0.0.1:11800".to_string(),
        Policy::System,
    );
    Ini::add(
        SKYWALKING_AGENT_SERVICE_NAME,
        "hello-skywalking".to_string(),
        Policy::System,
    );
    Ini::add(SKYWALKING_AGENT_WORKER_THREADS, 0i64, Policy::System);
    Ini::add(
        SKYWALKING_AGENT_LOG_LEVEL,
        "OFF".to_string(),
        Policy::System,
    );
    Ini::add(
        SKYWALKING_AGENT_LOG_FILE,
        "/tmp/skywalking-agent.log".to_string(),
        Policy::System,
    );

    // Hooks.
    module.on_module_init(module::init);
    module.on_module_shutdown(module::shutdown);
    module.on_request_init(request::init);
    module.on_request_shutdown(request::shutdown);

    // TODO Add swoole in future.
    // The function is used by swoole plugin, to surround the callback of on
    // request.
    // module.add_function(
    //     "skywalking_hack_swoole_on_request_please_do_not_use",
    //     request::skywalking_hack_swoole_on_request,
    //     vec![],
    // );

    module
}

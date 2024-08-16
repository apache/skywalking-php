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

#![allow(missing_docs)]
#![warn(rust_2018_idioms)]
#![warn(clippy::dbg_macro, clippy::print_stdout)]

mod channel;
mod component;
mod context;
mod errors;
mod execute;
mod module;
mod plugin;
mod request;
mod tag;
mod util;
mod worker;

use phper::{ini::Policy, modules::Module, php_get_module};

use crate::request::HACK_SWOOLE_ON_REQUEST_FUNCTION_NAME;
pub use errors::{Error, Result};

/// Enable agent and report or not.
const SKYWALKING_AGENT_ENABLE: &str = "skywalking_agent.enable";

/// Version of skywalking server.
const SKYWALKING_AGENT_SKYWALKING_VERSION: &str = "skywalking_agent.skywalking_version";

/// skywalking server address.
const SKYWALKING_AGENT_SERVER_ADDR: &str = "skywalking_agent.server_addr";

/// skywalking instance name.
const SKYWALKING_AGENT_INSTANCE_NAME: &str = "skywalking_agent.instance_name";

/// skywalking app service name.
const SKYWALKING_AGENT_SERVICE_NAME: &str = "skywalking_agent.service_name";

/// Tokio runtime worker threads.
const SKYWALKING_AGENT_WORKER_THREADS: &str = "skywalking_agent.worker_threads";

/// Log level of skywalking agent.
const SKYWALKING_AGENT_LOG_LEVEL: &str = "skywalking_agent.log_level";

/// Log file of skywalking agent.
const SKYWALKING_AGENT_LOG_FILE: &str = "skywalking_agent.log_file";

/// Skywalking agent runtime directory.
const SKYWALKING_AGENT_RUNTIME_DIR: &str = "skywalking_agent.runtime_dir";

/// Skywalking agent authentication token.
const SKYWALKING_AGENT_AUTHENTICATION: &str = "skywalking_agent.authentication";

/// Wether to enable tls for gPRC.
const SKYWALKING_AGENT_ENABLE_TLS: &str = "skywalking_agent.enable_tls";

/// The gRPC SSL trusted ca file.
const SKYWALKING_AGENT_SSL_TRUSTED_CA_PATH: &str = "skywalking_agent.ssl_trusted_ca_path";

/// The private key file. Enable mTLS when ssl_key_path and ssl_cert_chain_path
/// exist.
const SKYWALKING_AGENT_SSL_KEY_PATH: &str = "skywalking_agent.ssl_key_path";

/// The certificate file. Enable mTLS when ssl_key_path and ssl_cert_chain_path
/// exist.
const SKYWALKING_AGENT_SSL_CERT_CHAIN_PATH: &str = "skywalking_agent.ssl_cert_chain_path";

/// Agent heartbeat report period. Unit, second.
const SKYWALKING_AGENT_HEARTBEAT_PERIOD: &str = "skywalking_agent.heartbeat_period";

/// The agent sends the instance properties to the backend every
/// heartbeat_period * properties_report_period_factor seconds.
const SKYWALKING_AGENT_PROPERTIES_REPORT_PERIOD_FACTOR: &str =
    "skywalking_agent.properties_report_period_factor";

/// Whether to use zend observer instead of zend_execute_ex to hook the
/// functions. This feature is only available for PHP8+, and can work with
/// PHP8's jit.
const SKYWALKING_AGENT_ENABLE_ZEND_OBSERVER: &str = "skywalking_agent.enable_zend_observer";

/// Reporter type, optional values are `grpc`, `kafka` and `standalone`, default
/// is `grpc`.
const SKYWALKING_AGENT_REPORTER_TYPE: &str = "skywalking_agent.reporter_type";

/// A list of host/port pairs to use for establishing the initial connection to
/// the Kafka cluster. Only available when the reporter type is `kafka`.
const SKYWALKING_AGENT_KAFKA_BOOTSTRAP_SERVERS: &str = "skywalking_agent.kafka_bootstrap_servers";

/// Configure Kafka Producer configuration in JSON format.
/// Only available when the reporter type is `kafka`.
const SKYWALKING_AGENT_KAFKA_PRODUCER_CONFIG: &str = "skywalking_agent.kafka_producer_config";

/// Whether to enable automatic injection of skywalking context variables (such
/// as `SW_TRACE_ID`). For `php-fpm` mode, it will be injected into the
/// `$_SERVER` variable. For `swoole` mode, it will be injected into the
/// `$request->server` variable.
const SKYWALKING_AGENT_INJECT_CONTEXT: &str = "skywalking_agent.inject_context";

/// Unix domain socket file path of standalone skywalking php worker. Only
/// available when `reporter_type` is `standalone`.
const SKYWALKING_AGENT_STANDALONE_SOCKET_PATH: &str = "skywalking_agent.standalone_socket_path";

#[php_get_module]
pub fn get_module() -> Module {
    let mut module = Module::new(
        env!("CARGO_CRATE_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_AUTHORS"),
    );

    // Register skywalking ini.
    module.add_ini(SKYWALKING_AGENT_ENABLE, false, Policy::System);
    module.add_ini(SKYWALKING_AGENT_SKYWALKING_VERSION, 8i64, Policy::System);
    module.add_ini(
        SKYWALKING_AGENT_SERVER_ADDR,
        "127.0.0.1:11800".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_INSTANCE_NAME,
        "".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_SERVICE_NAME,
        "hello-skywalking".to_string(),
        Policy::System,
    );
    module.add_ini(SKYWALKING_AGENT_WORKER_THREADS, 0i64, Policy::System);
    module.add_ini(
        SKYWALKING_AGENT_LOG_LEVEL,
        "OFF".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_LOG_FILE,
        "/tmp/skywalking-agent.log".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_RUNTIME_DIR,
        "/tmp/skywalking-agent".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_AUTHENTICATION,
        "".to_string(),
        Policy::System,
    );
    module.add_ini(SKYWALKING_AGENT_ENABLE_TLS, false, Policy::System);
    module.add_ini(
        SKYWALKING_AGENT_SSL_TRUSTED_CA_PATH,
        "".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_SSL_KEY_PATH,
        "".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_SSL_CERT_CHAIN_PATH,
        "".to_string(),
        Policy::System,
    );
    module.add_ini(SKYWALKING_AGENT_HEARTBEAT_PERIOD, 30i64, Policy::System);
    module.add_ini(
        SKYWALKING_AGENT_PROPERTIES_REPORT_PERIOD_FACTOR,
        10i64,
        Policy::System,
    );
    module.add_ini(SKYWALKING_AGENT_ENABLE_ZEND_OBSERVER, false, Policy::System);
    module.add_ini(
        SKYWALKING_AGENT_REPORTER_TYPE,
        "grpc".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_KAFKA_BOOTSTRAP_SERVERS,
        "".to_string(),
        Policy::System,
    );
    module.add_ini(
        SKYWALKING_AGENT_KAFKA_PRODUCER_CONFIG,
        "{}".to_string(),
        Policy::System,
    );
    module.add_ini(SKYWALKING_AGENT_INJECT_CONTEXT, false, Policy::System);
    module.add_ini(
        SKYWALKING_AGENT_STANDALONE_SOCKET_PATH,
        "".to_string(),
        Policy::System,
    );

    // Hooks.
    module.on_module_init(module::init);
    module.on_module_shutdown(module::shutdown);
    module.on_request_init(request::init);
    module.on_request_shutdown(request::shutdown);

    // The function is used by swoole plugin, to surround the callback of on
    // request.
    module.add_function(
        HACK_SWOOLE_ON_REQUEST_FUNCTION_NAME,
        request::skywalking_hack_swoole_on_request,
    );

    module
}

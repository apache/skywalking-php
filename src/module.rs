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

use crate::{
    channel::Reporter,
    execute::{register_execute_functions, register_observer_handlers},
    util::{get_sapi_module_name, get_str_ini_with_default, IPS},
    worker::init_worker,
    *,
};
use anyhow::bail;
use once_cell::sync::Lazy;
use phper::{arrays::ZArr, ini::ini_get, sys};
use skywalking::{
    common::random_generator::RandomGenerator,
    trace::tracer::{self, Tracer},
};
use std::{
    ffi::{CStr, OsStr},
    fs::{self, OpenOptions},
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};
use tracing::{debug, error, info, metadata::LevelFilter};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

static IS_ENABLE: Lazy<bool> = Lazy::new(|| {
    if !ini_get::<bool>(SKYWALKING_AGENT_ENABLE) {
        return false;
    }

    let sapi = get_sapi_module_name().to_bytes();

    if sapi == b"fpm-fcgi" {
        return true;
    }

    if sapi == b"cli" && get_module_registry().exists("swoole") {
        return true;
    }

    false
});

pub static SERVER_ADDR: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_SERVER_ADDR));

pub static SERVICE_NAME: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_SERVICE_NAME));

pub static SERVICE_INSTANCE: Lazy<String> = Lazy::new(|| {
    let rnd_hostname = RandomGenerator::generate() + "@" + &IPS[0];
    let mut service_instance = rnd_hostname.as_str();

    let defined_instance_name = ini_get::<Option<&CStr>>(SKYWALKING_AGENT_INSTANCE_NAME)
        .and_then(|s| s.to_str().ok())
        .unwrap_or_default();
    let defined_instance_name = defined_instance_name.trim();

    if !defined_instance_name.is_empty() {
        service_instance = defined_instance_name;
    }
    service_instance.to_string()
});

pub static SKYWALKING_VERSION: Lazy<i64> =
    Lazy::new(|| ini_get::<i64>(SKYWALKING_AGENT_SKYWALKING_VERSION));

pub static RUNTIME_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = PathBuf::new();
    if let Some(dir) = ini_get::<Option<&CStr>>(SKYWALKING_AGENT_RUNTIME_DIR) {
        let dir = dir.to_bytes();
        if !dir.is_empty() {
            path.push(OsStr::from_bytes(dir));
        }
    }
    path
});

pub static SOCKET_FILE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut dir = RUNTIME_DIR.clone();

    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Get timestamp failed")
        .as_micros();

    dir.push(format!("{:x}.sock", dur));

    dir
});

pub static AUTHENTICATION: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_AUTHENTICATION));

pub static ENABLE_TLS: Lazy<bool> = Lazy::new(|| ini_get::<bool>(SKYWALKING_AGENT_ENABLE_TLS));

pub static SSL_TRUSTED_CA_PATH: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_SSL_TRUSTED_CA_PATH));

pub static SSL_KEY_PATH: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_SSL_KEY_PATH));

pub static SSL_CERT_CHAIN_PATH: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_SSL_CERT_CHAIN_PATH));

pub static HEARTBEAT_PERIOD: Lazy<i64> =
    Lazy::new(|| ini_get::<i64>(SKYWALKING_AGENT_HEARTBEAT_PERIOD));

pub static PROPERTIES_REPORT_PERIOD_FACTOR: Lazy<i64> =
    Lazy::new(|| ini_get::<i64>(SKYWALKING_AGENT_PROPERTIES_REPORT_PERIOD_FACTOR));

/// Zend observer is only support in PHP8+.
pub static ENABLE_ZEND_OBSERVER: Lazy<bool> = Lazy::new(|| {
    sys::PHP_MAJOR_VERSION >= 8 && ini_get::<bool>(SKYWALKING_AGENT_ENABLE_ZEND_OBSERVER)
});

pub static WORKER_THREADS: Lazy<i64> =
    Lazy::new(|| ini_get::<i64>(SKYWALKING_AGENT_WORKER_THREADS));

pub static REPORTER_TYPE: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_REPORTER_TYPE));

pub static KAFKA_BOOTSTRAP_SERVERS: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_KAFKA_BOOTSTRAP_SERVERS));

pub static KAFKA_PRODUCER_CONFIG: Lazy<String> =
    Lazy::new(|| get_str_ini_with_default(SKYWALKING_AGENT_KAFKA_PRODUCER_CONFIG));

pub static INJECT_CONTEXT: Lazy<bool> =
    Lazy::new(|| ini_get::<bool>(SKYWALKING_AGENT_INJECT_CONTEXT));

/// For PHP 8.2+, zend observer api are now also called for internal functions.
///
/// Refer to this commit: <https://github.com/php/php-src/commit/625f1649639c2b9a9d76e4d42f88c264ddb8447d>
#[allow(clippy::absurd_extreme_comparisons)]
pub const IS_ZEND_OBSERVER_CALLED_FOR_INTERNAL: bool =
    sys::PHP_MAJOR_VERSION > 8 || (sys::PHP_MAJOR_VERSION == 8 && sys::PHP_MINOR_VERSION >= 2);

pub fn init() {
    if !is_enable() {
        return;
    }

    // Initialize configuration properties.
    Lazy::force(&SERVER_ADDR);
    Lazy::force(&SERVICE_NAME);
    Lazy::force(&SERVICE_INSTANCE);
    Lazy::force(&SKYWALKING_VERSION);
    Lazy::force(&RUNTIME_DIR);
    Lazy::force(&SOCKET_FILE_PATH);
    Lazy::force(&AUTHENTICATION);
    Lazy::force(&ENABLE_TLS);
    Lazy::force(&SSL_TRUSTED_CA_PATH);
    Lazy::force(&SSL_KEY_PATH);
    Lazy::force(&SSL_CERT_CHAIN_PATH);
    Lazy::force(&HEARTBEAT_PERIOD);
    Lazy::force(&PROPERTIES_REPORT_PERIOD_FACTOR);
    Lazy::force(&ENABLE_ZEND_OBSERVER);
    Lazy::force(&WORKER_THREADS);
    Lazy::force(&REPORTER_TYPE);
    Lazy::force(&KAFKA_BOOTSTRAP_SERVERS);
    Lazy::force(&KAFKA_PRODUCER_CONFIG);
    Lazy::force(&INJECT_CONTEXT);

    if let Err(err) = try_init_logger() {
        eprintln!("skywalking_agent: initialize logger failed: {}", err);
    }

    // Skywalking agent info.
    info!(
        service_name = &*SERVICE_NAME,
        service_instance = &*SERVICE_INSTANCE,
        skywalking_version = &*SKYWALKING_VERSION,
        heartbeat_period = &*HEARTBEAT_PERIOD,
        properties_report_period_factor = &*PROPERTIES_REPORT_PERIOD_FACTOR,
        "Starting skywalking agent"
    );

    // Skywalking version check.
    let skywalking_version = *SKYWALKING_VERSION;
    if skywalking_version < 8 {
        error!(
            skywalking_version,
            "The skywalking agent only supports versions after skywalking 8"
        );
        return;
    }

    // Initialize runtime directory.
    if RUNTIME_DIR.as_os_str().is_empty() {
        error!("The skywalking agent runtime directory must not be empty");
        return;
    }
    if let Err(err) = fs::create_dir_all(&*RUNTIME_DIR) {
        error!(?err, "Create runtime directory failed");
        return;
    }

    // Initialize Agent worker.
    init_worker();

    tracer::set_global_tracer(Tracer::new(
        &*SERVICE_NAME,
        &*SERVICE_INSTANCE,
        Reporter::new(&*SOCKET_FILE_PATH),
    ));

    // Hook functions.
    register_execute_functions();
    register_observer_handlers();
}

pub fn shutdown() {
    if !is_enable() {
        return;
    }

    debug!("skywalking agent shutdown hook called");
}

fn try_init_logger() -> anyhow::Result<()> {
    let log_level = ini_get::<Option<&CStr>>(SKYWALKING_AGENT_LOG_LEVEL)
        .and_then(|s| s.to_str().ok())
        .unwrap_or("OFF");
    let log_level = log_level.trim();

    let log_level = LevelFilter::from_str(log_level)?;
    if log_level == LevelFilter::OFF {
        return Ok(());
    }

    let log_file = ini_get::<Option<&CStr>>(SKYWALKING_AGENT_LOG_FILE)
        .and_then(|s| s.to_str().ok())
        .unwrap_or_default();
    let log_file = log_file.trim();
    if log_file.is_empty() {
        bail!("log file cant't be empty when log enabled");
    }

    let path = Path::new(log_file);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut open_options = OpenOptions::new();
    open_options.append(true).create(true);

    let file = open_options.open(path)?;

    let filter = EnvFilter::new(format!("info,skywalking_agent={}", log_level));

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(file)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

#[inline]
fn get_module_registry() -> &'static ZArr {
    unsafe { ZArr::from_ptr(&sys::module_registry) }
}

#[inline]
pub fn is_enable() -> bool {
    *IS_ENABLE
}

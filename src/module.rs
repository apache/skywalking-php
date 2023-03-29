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
    execute::register_execute_functions,
    util::{get_sapi_module_name, IPS},
    worker::init_worker,
    SKYWALKING_AGENT_AUTHENTICATION, SKYWALKING_AGENT_ENABLE, SKYWALKING_AGENT_ENABLE_TLS,
    SKYWALKING_AGENT_HEARTBEAT_PERIOD, SKYWALKING_AGENT_LOG_FILE, SKYWALKING_AGENT_LOG_LEVEL,
    SKYWALKING_AGENT_PROPERTIES_REPORT_PERIOD_FACTOR, SKYWALKING_AGENT_RUNTIME_DIR,
    SKYWALKING_AGENT_SERVICE_NAME, SKYWALKING_AGENT_SKYWALKING_VERSION,
    SKYWALKING_AGENT_SSL_CERT_CHAIN_PATH, SKYWALKING_AGENT_SSL_KEY_PATH,
    SKYWALKING_AGENT_SSL_TRUSTED_CA_PATH,
};
use anyhow::bail;
use once_cell::sync::Lazy;
use phper::{arrays::ZArr, ini::ini_get, sys};
use skywalking::{
    common::random_generator::RandomGenerator,
    trace::tracer::{self, Tracer},
};
use std::{
    borrow::ToOwned,
    ffi::{CStr, OsStr},
    fs::{self, OpenOptions},
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};
use tracing::{debug, error, info, metadata::LevelFilter};
use tracing_subscriber::FmtSubscriber;

pub static SERVICE_NAME: Lazy<String> = Lazy::new(|| {
    ini_get::<Option<&CStr>>(SKYWALKING_AGENT_SERVICE_NAME)
        .and_then(|s| s.to_str().ok())
        .map(ToOwned::to_owned)
        .unwrap_or_default()
});

pub static SERVICE_INSTANCE: Lazy<String> =
    Lazy::new(|| RandomGenerator::generate() + "@" + &IPS[0]);

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

pub static AUTHENTICATION: Lazy<String> = Lazy::new(|| {
    ini_get::<Option<&CStr>>(SKYWALKING_AGENT_AUTHENTICATION)
        .and_then(|s| s.to_str().ok())
        .map(ToOwned::to_owned)
        .unwrap_or_default()
});

pub static ENABLE_TLS: Lazy<bool> = Lazy::new(|| ini_get::<bool>(SKYWALKING_AGENT_ENABLE_TLS));

pub static SSL_TRUSTED_CA_PATH: Lazy<String> = Lazy::new(|| {
    ini_get::<Option<&CStr>>(SKYWALKING_AGENT_SSL_TRUSTED_CA_PATH)
        .and_then(|s| s.to_str().ok())
        .map(ToOwned::to_owned)
        .unwrap_or_default()
});

pub static SSL_KEY_PATH: Lazy<String> = Lazy::new(|| {
    ini_get::<Option<&CStr>>(SKYWALKING_AGENT_SSL_KEY_PATH)
        .and_then(|s| s.to_str().ok())
        .map(ToOwned::to_owned)
        .unwrap_or_default()
});

pub static SSL_CERT_CHAIN_PATH: Lazy<String> = Lazy::new(|| {
    ini_get::<Option<&CStr>>(SKYWALKING_AGENT_SSL_CERT_CHAIN_PATH)
        .and_then(|s| s.to_str().ok())
        .map(ToOwned::to_owned)
        .unwrap_or_default()
});

pub static HEARTBEAT_PERIOD: Lazy<i64> =
    Lazy::new(|| ini_get::<i64>(SKYWALKING_AGENT_HEARTBEAT_PERIOD));

pub static PROPERTIES_REPORT_PERIOD_FACTOR: Lazy<i64> =
    Lazy::new(|| ini_get::<i64>(SKYWALKING_AGENT_PROPERTIES_REPORT_PERIOD_FACTOR));

pub fn init() {
    if !is_enable() {
        return;
    }

    if let Err(err) = try_init_logger() {
        eprintln!("skywalking_agent: initialize logger failed: {}", err);
    }

    // Skywalking agent info.
    let service_name = Lazy::force(&SERVICE_NAME);
    let service_instance = Lazy::force(&SERVICE_INSTANCE);
    let skywalking_version = Lazy::force(&SKYWALKING_VERSION);
    let authentication = Lazy::force(&AUTHENTICATION);
    let heartbeat_period = Lazy::force(&HEARTBEAT_PERIOD);
    let properties_report_period_factor = Lazy::force(&PROPERTIES_REPORT_PERIOD_FACTOR);
    info!(
        service_name,
        service_instance,
        skywalking_version,
        authentication,
        heartbeat_period,
        properties_report_period_factor,
        "Starting skywalking agent"
    );

    // Skywalking version check.
    if *skywalking_version < 8 {
        error!(
            skywalking_version,
            "The skywalking agent only supports versions after skywalking 8"
        );
        return;
    }

    // Initialize TLS if enabled.
    let enable_tls = Lazy::force(&ENABLE_TLS);
    let ssl_trusted_ca_path = Lazy::force(&SSL_TRUSTED_CA_PATH);
    let ssl_key_path = Lazy::force(&SSL_KEY_PATH);
    let ssl_cert_chain_path = Lazy::force(&SSL_CERT_CHAIN_PATH);
    debug!(
        enable_tls,
        ssl_trusted_ca_path, ssl_key_path, ssl_cert_chain_path, "Skywalking TLS info"
    );

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
    Lazy::force(&SOCKET_FILE_PATH);
    init_worker();

    tracer::set_global_tracer(Tracer::new(
        service_name,
        service_instance,
        Reporter::new(&*SOCKET_FILE_PATH),
    ));

    // Hook functions.
    register_execute_functions();
}

pub fn shutdown() {
    if !is_enable() {
        return;
    }

    info!("Shutdowning skywalking agent");
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

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_ansi(false)
        .with_writer(file)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

fn get_module_registry() -> &'static ZArr {
    unsafe { ZArr::from_ptr(&sys::module_registry) }
}

pub fn is_enable() -> bool {
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
    *IS_ENABLE
}

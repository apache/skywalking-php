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
    SKYWALKING_AGENT_ENABLE, SKYWALKING_AGENT_LOG_FILE, SKYWALKING_AGENT_LOG_LEVEL,
    SKYWALKING_AGENT_RUNTIME_DIR, SKYWALKING_AGENT_SERVICE_NAME,
    SKYWALKING_AGENT_SKYWALKING_VERSION,
};
use once_cell::sync::Lazy;
use phper::{arrays::ZArr, ini::ini_get, sys};
use skywalking::{
    common::random_generator::RandomGenerator,
    trace::tracer::{self, Tracer},
};
use std::{
    borrow::ToOwned,
    ffi::{CStr, OsStr},
    fs,
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};
use tracing::{error, info, metadata::LevelFilter};
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

pub fn init() {
    if !is_enable() {
        return;
    }

    init_logger();

    let service_name = Lazy::force(&SERVICE_NAME);
    let service_instance = Lazy::force(&SERVICE_INSTANCE);
    let skywalking_version = Lazy::force(&SKYWALKING_VERSION);
    info!(
        service_name,
        service_instance, skywalking_version, "Starting skywalking agent"
    );

    // Skywalking version check
    if *skywalking_version < 8 {
        error!(
            skywalking_version,
            "The skywalking agent only supports versions after skywalking 8"
        );
        return;
    }

    if RUNTIME_DIR.as_os_str().is_empty() {
        error!("The skywalking agent runtime directory must not be empty");
        return;
    }
    if let Err(err) = fs::create_dir_all(&*RUNTIME_DIR) {
        error!(?err, "Create runtime directory failed");
        return;
    }

    Lazy::force(&SOCKET_FILE_PATH);
    init_worker();

    tracer::set_global_tracer(Tracer::new(
        service_name,
        service_instance,
        Reporter::new(&*SOCKET_FILE_PATH),
    ));

    register_execute_functions();
}

pub fn shutdown() {}

fn init_logger() {
    let log_level = ini_get::<Option<&CStr>>(SKYWALKING_AGENT_LOG_LEVEL)
        .and_then(|s| s.to_str().ok())
        .unwrap_or("OFF");
    let log_level = log_level.trim();

    let log_file = ini_get::<Option<&CStr>>(SKYWALKING_AGENT_LOG_FILE)
        .and_then(|s| s.to_str().ok())
        .unwrap_or_default();
    let log_file = log_file.trim();

    if !log_file.is_empty() {
        if let Ok(log_level) = LevelFilter::from_str(log_level) {
            let log_file = Path::new(log_file);
            if let Some(dir) = log_file.parent() {
                if let Some(file_name) = log_file.file_name() {
                    let file_appender = tracing_appender::rolling::never(dir, file_name);
                    let subscriber = FmtSubscriber::builder()
                        .with_max_level(log_level)
                        .with_ansi(false)
                        .with_writer(file_appender)
                        .finish();

                    tracing::subscriber::set_global_default(subscriber)
                        .expect("setting default subscriber failed");
                }
            }
        }
    }
}

fn get_module_registry() -> &'static ZArr {
    unsafe { ZArr::from_ptr(&sys::module_registry) }
}

fn is_enable() -> bool {
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
}

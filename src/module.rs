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
    worker::{init_worker, shutdown_worker},
    SKYWALKING_AGENT_ENABLE, SKYWALKING_AGENT_LOG_FILE, SKYWALKING_AGENT_LOG_LEVEL,
    SKYWALKING_AGENT_SERVICE_NAME, SKYWALKING_AGENT_SKYWALKING_VERSION,
};
use once_cell::sync::Lazy;
use phper::{arrays::ZArr, ini::Ini, modules::ModuleContext, sys};
use skywalking::{
    common::random_generator::RandomGenerator,
    trace::tracer::{self, Tracer},
};
use std::{path::Path, str::FromStr};

use tracing::{error, info, metadata::LevelFilter};
use tracing_subscriber::FmtSubscriber;

pub static SERVICE_NAME: Lazy<String> =
    Lazy::new(|| Ini::get::<String>(SKYWALKING_AGENT_SERVICE_NAME).unwrap_or_default());

pub static SERVICE_INSTANCE: Lazy<String> =
    Lazy::new(|| RandomGenerator::generate() + "@" + &IPS[0]);

pub static SKYWALKING_VERSION: Lazy<i64> =
    Lazy::new(|| Ini::get::<i64>(SKYWALKING_AGENT_SKYWALKING_VERSION).unwrap_or_default());

pub fn init(_module: ModuleContext) -> bool {
    if !is_enable() {
        return true;
    }

    init_logger();

    let service_name = Lazy::force(&SERVICE_NAME);
    let service_instance = Lazy::force(&SERVICE_INSTANCE);
    let skywalking_version = Lazy::force(&SKYWALKING_VERSION);
    info!(
        service_name,
        service_instance, skywalking_version, "Starting skywalking agent"
    );

    let worker_addr = {
        match tempfile::NamedTempFile::new() {
            Err(e) => {
                error!("Create named temporary file failed: {}", e);
                return true;
            }
            Ok(f) => match f.into_temp_path().to_str() {
                None => {
                    error!("Yields a &str slice from the Path failed.");
                    return true;
                }
                Some(s) => s.to_string(),
            },
        }
    };

    init_worker(&worker_addr);

    tracer::set_global_tracer(Tracer::new(
        service_name,
        service_instance,
        Reporter::new(worker_addr),
    ));

    register_execute_functions();

    true
}

pub fn shutdown(_module: ModuleContext) -> bool {
    shutdown_worker();

    true
}

fn init_logger() {
    let log_level =
        Ini::get::<String>(SKYWALKING_AGENT_LOG_LEVEL).unwrap_or_else(|| "OFF".to_string());
    let log_level = log_level.trim();

    let log_file = Ini::get::<String>(SKYWALKING_AGENT_LOG_FILE).unwrap_or_else(|| "".to_string());
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
    if !Ini::get::<bool>(SKYWALKING_AGENT_ENABLE).unwrap_or_default() {
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

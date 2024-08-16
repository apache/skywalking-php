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

use crate::module::{
    is_standalone_reporter_type, AUTHENTICATION, ENABLE_TLS, HEARTBEAT_PERIOD,
    PROPERTIES_REPORT_PERIOD_FACTOR, REPORTER_TYPE, SERVER_ADDR, SERVICE_INSTANCE, SERVICE_NAME,
    SOCKET_FILE_PATH, SSL_CERT_CHAIN_PATH, SSL_KEY_PATH, SSL_TRUSTED_CA_PATH, WORKER_THREADS,
};
#[cfg(feature = "kafka-reporter")]
use crate::module::{KAFKA_BOOTSTRAP_SERVERS, KAFKA_PRODUCER_CONFIG};
#[cfg(feature = "kafka-reporter")]
use skywalking_php_worker::reporter::KafkaReporterConfiguration;
use skywalking_php_worker::{
    new_tokio_runtime,
    reporter::{GrpcReporterConfiguration, ReporterConfiguration},
    start_worker, HeartBeatConfiguration, WorkerConfiguration,
};
use std::{cmp::Ordering, num::NonZeroUsize, process::exit, thread::available_parallelism};
use tracing::error;

pub fn init_worker() {
    if is_standalone_reporter_type() {
        return;
    }

    unsafe {
        // TODO Shutdown previous worker before fork if there is a PHP-FPM reload
        // operation.
        // TODO Change the worker process name.

        let pid = libc::fork();
        match pid.cmp(&0) {
            Ordering::Less => {
                error!("fork failed");
            }
            Ordering::Equal => {
                // Ensure worker process exits when master process exists.
                #[cfg(target_os = "linux")]
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);

                let reporter_config = match REPORTER_TYPE.as_str() {
                    "grpc" => ReporterConfiguration::Grpc(GrpcReporterConfiguration {
                        authentication: AUTHENTICATION.clone(),
                        enable_tls: *ENABLE_TLS,
                        server_addr: SERVER_ADDR.clone(),
                        ssl_cert_chain_path: SSL_CERT_CHAIN_PATH.clone(),
                        ssl_key_path: SSL_KEY_PATH.clone(),
                        ssl_trusted_ca_path: SSL_TRUSTED_CA_PATH.clone(),
                    }),
                    #[cfg(feature = "kafka-reporter")]
                    "kafka" => ReporterConfiguration::Kafka(KafkaReporterConfiguration {
                        kafka_bootstrap_servers: KAFKA_BOOTSTRAP_SERVERS.clone(),
                        kafka_producer_config: KAFKA_PRODUCER_CONFIG.clone(),
                    }),
                    typ => {
                        error!("unknown reporter type, {}", typ);
                        exit(1);
                    }
                };

                let config = WorkerConfiguration {
                    socket_file_path: SOCKET_FILE_PATH.to_path_buf(),
                    heart_beat: Some(HeartBeatConfiguration {
                        service_instance: SERVICE_INSTANCE.clone(),
                        service_name: SERVICE_NAME.clone(),
                        heartbeat_period: *HEARTBEAT_PERIOD,
                        properties_report_period_factor: *PROPERTIES_REPORT_PERIOD_FACTOR,
                    }),
                    reporter_config,
                };

                // Run the worker in subprocess.
                let rt = new_tokio_runtime(worker_threads());
                match rt.block_on(start_worker(config)) {
                    Ok(_) => {
                        exit(0);
                    }
                    Err(err) => {
                        error!(?err, "worker exit unexpectedly");
                        exit(1);
                    }
                }
            }
            Ordering::Greater => {}
        }
    }
}

fn worker_threads() -> usize {
    let worker_threads = *WORKER_THREADS;
    if worker_threads <= 0 {
        available_parallelism().map(NonZeroUsize::get).unwrap_or(1)
    } else {
        worker_threads as usize
    }
}

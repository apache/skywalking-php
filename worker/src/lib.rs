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

pub mod channel;
pub mod reporter;

use crate::{
    channel::TxReporter,
    reporter::{ReporterConfiguration, run_reporter},
};
use skywalking::{
    management::{instance::Properties, manager::Manager},
    reporter::{CollectItem, CollectItemConsume},
};
use std::{
    error::Error,
    fs, io,
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    net::UnixListener,
    runtime::{self, Runtime},
    select,
    signal::unix::{SignalKind, signal},
    sync::mpsc::{self, error::TrySendError},
};
use tonic::async_trait;
use tracing::{debug, error, info};

pub struct WorkerConfiguration {
    pub socket_file_path: PathBuf,
    pub heart_beat: Option<HeartBeatConfiguration>,
    pub reporter_config: ReporterConfiguration,
}

pub struct HeartBeatConfiguration {
    pub service_instance: String,
    pub service_name: String,
    pub heartbeat_period: i64,
    pub properties_report_period_factor: i64,
}

pub fn new_tokio_runtime(worker_threads: usize) -> Runtime {
    runtime::Builder::new_multi_thread()
        .thread_name("sw: worker")
        .enable_all()
        .worker_threads(worker_threads)
        .build()
        .unwrap()
}

pub async fn start_worker(config: WorkerConfiguration) -> anyhow::Result<()> {
    debug!("Starting worker...");

    let socket_file = config.socket_file_path;

    // Ensure to cleanup resources when worker exits.
    let _guard = WorkerExitGuard(socket_file.clone());

    // Graceful shutdown signal, put it on the top of program.
    let mut sig_term = signal(SignalKind::terminate())?;
    let mut sig_int = signal(SignalKind::interrupt())?;

    let fut = async move {
        debug!(?socket_file, "Bind unix stream");
        let listener = UnixListener::bind(&socket_file)?;
        change_permission(socket_file, 0o777);

        let (tx, rx) = mpsc::channel::<CollectItem>(255);
        let tx_ = tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, _addr)) => {
                        let tx = tx.clone();

                        tokio::spawn(async move {
                            debug!("Entering channel_receive loop");

                            loop {
                                let r = match channel::channel_receive(&mut stream).await {
                                    Err(err) => match err.downcast_ref::<io::Error>() {
                                        Some(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                                            debug!("Leaving channel_receive loop");
                                            return;
                                        }
                                        _ => {
                                            error!(?err, "channel_receive failed");
                                            continue;
                                        }
                                    },
                                    Ok(i) => i,
                                };

                                // Try send here, to prevent the ipc blocking caused by the channel
                                // bursting (too late to report),
                                // which affects the pool process of php-fpm.
                                if let Err(err) = tx.try_send(r) {
                                    error!(?err, "Send collect item failed");
                                    if !matches!(err, TrySendError::Full(_)) {
                                        return;
                                    }
                                }
                            }
                        });
                    }
                    Err(err) => {
                        error!(?err, "Accept failed");
                    }
                }
            }
        });

        if let Some(heart_beat_config) = config.heart_beat {
            report_properties_and_keep_alive(heart_beat_config, TxReporter(tx_));
        }

        // Run reporter with blocking.
        run_reporter(config.reporter_config, (), Consumer(rx)).await?;

        Ok::<_, anyhow::Error>(())
    };

    // TODO Do graceful shutdown, and wait 10s then force quit.
    select! {
        _ = sig_term.recv() => {}
        _ = sig_int.recv() => {}
        r = fut => {
            r?;
        }
    }

    info!("Start to shutdown skywalking grpc reporter");

    Ok(())
}

struct Consumer(mpsc::Receiver<CollectItem>);

#[async_trait]
impl CollectItemConsume for Consumer {
    async fn consume(&mut self) -> Result<Option<CollectItem>, Box<dyn Error + Send>> {
        Ok(self.0.recv().await)
    }

    async fn try_consume(&mut self) -> Result<Option<CollectItem>, Box<dyn Error + Send>> {
        Ok(self.0.try_recv().ok())
    }
}

struct WorkerExitGuard(PathBuf);

impl Drop for WorkerExitGuard {
    fn drop(&mut self) {
        let &mut Self(ref socket_file) = self;
        info!(?socket_file, "Remove socket file");
        if let Err(err) = fs::remove_file(socket_file) {
            error!(?err, "Remove socket file failed");
        }
    }
}

fn report_properties_and_keep_alive(config: HeartBeatConfiguration, reporter: TxReporter) {
    let manager = Manager::new(&*config.service_name, &*config.service_instance, reporter);

    manager.report_and_keep_alive(
        || {
            let mut props = Properties::new();
            props.insert_os_info();
            props.update(Properties::KEY_LANGUAGE, "php");
            props.update(Properties::KEY_PROCESS_NO, unsafe {
                libc::getppid().to_string()
            });
            debug!(?props, "Report instance properties");
            props
        },
        Duration::from_secs(config.heartbeat_period as u64),
        config.properties_report_period_factor as usize,
    );
}

fn change_permission(f: impl AsRef<Path>, mode: libc::mode_t) {
    let f = f.as_ref().as_os_str().as_bytes();
    let mut path = Vec::with_capacity(f.len() + 1);
    path.extend_from_slice(f);
    path.push(b'\0');
    unsafe {
        libc::chmod(path.as_ptr().cast(), mode);
    }
}

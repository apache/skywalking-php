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
    channel, module::SOCKET_FILE_PATH, util::change_permission, SKYWALKING_AGENT_SERVER_ADDR,
    SKYWALKING_AGENT_WORKER_THREADS,
};
use once_cell::sync::{Lazy, OnceCell};
use phper::ini::ini_get;
use skywalking::reporter::{
    grpc::{ColletcItemConsume, GrpcReporter},
    CollectItem,
};
use std::{
    cmp::Ordering, error::Error, ffi::CStr, fs, io, num::NonZeroUsize, process::exit,
    thread::available_parallelism, time::Duration,
};
use tokio::{
    net::UnixListener,
    runtime::{self, Runtime},
    select,
    signal::unix::{signal, SignalKind},
    sync::mpsc,
    time::sleep,
};
use tonic::{
    async_trait,
    transport::{Channel, Endpoint},
};
use tracing::{debug, error, info, warn};

static WORKER_PID: OnceCell<libc::pid_t> = OnceCell::new();

pub fn init_worker() {
    let server_addr = ini_get::<Option<&CStr>>(SKYWALKING_AGENT_SERVER_ADDR)
        .and_then(|s| s.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let worker_threads = worker_threads();

    unsafe {
        // TODO Shutdown previous worker before fork if there is a PHP-FPM reload
        // operation.
        // TODO Chagne the worker process name.

        let pid = libc::fork();
        match pid.cmp(&0) {
            Ordering::Less => {
                error!("fork failed");
            }
            Ordering::Equal => {
                let rt = new_tokio_runtime(worker_threads);
                rt.block_on(start_worker(server_addr));
                exit(0);
            }
            Ordering::Greater => {
                WORKER_PID.set(pid).unwrap();
            }
        }
    }
}

pub fn shutdown_worker() {
    if let Some(pid) = WORKER_PID.get() {
        debug!(pid, "Start to shutdown worker");
        unsafe {
            libc::kill(*pid, libc::SIGTERM);
        }
    }
}

fn worker_threads() -> usize {
    let worker_threads = ini_get::<i64>(SKYWALKING_AGENT_WORKER_THREADS);
    if worker_threads <= 0 {
        available_parallelism().map(NonZeroUsize::get).unwrap_or(1)
    } else {
        worker_threads as usize
    }
}

fn new_tokio_runtime(worker_threads: usize) -> Runtime {
    runtime::Builder::new_multi_thread()
        .thread_name("sw: worker")
        .enable_all()
        .worker_threads(worker_threads)
        .build()
        .unwrap()
}

async fn start_worker(server_addr: String) {
    debug!("Starting worker...");

    // Graceful shutdown signal, put it on the top of program.
    let mut sig_term = match signal(SignalKind::terminate()) {
        Ok(signal) => signal,
        Err(err) => {
            error!(?err, "Signal terminate failed");
            return;
        }
    };
    let mut sig_int = match signal(SignalKind::interrupt()) {
        Ok(signal) => signal,
        Err(err) => {
            error!(?err, "Signal interrupt failed");
            return;
        }
    };

    let socket_file = SOCKET_FILE_PATH.as_str();

    let fut = async move {
        debug!(socket_file, "Bind unix stream");
        let listener = match UnixListener::bind(socket_file) {
            Ok(listener) => listener,
            Err(err) => {
                error!(?err, "Bind failed");
                return;
            }
        };
        change_permission(socket_file, 0o777);

        let (tx, rx) = mpsc::channel::<Result<CollectItem, Box<dyn Error + Send>>>(255);
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
                                        _ => Err(err.into()),
                                    },
                                    Ok(i) => Ok(i),
                                };

                                if let Err(err) = tx.send(r).await {
                                    error!(?err, "Send failed");
                                    return;
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

        let endpoint = match Endpoint::from_shared(server_addr) {
            Ok(endpoint) => endpoint,
            Err(err) => {
                error!(?err, "Create endpoint failed");
                return;
            }
        };
        let channel = connect(endpoint).await;

        let reporter = GrpcReporter::new_with_pc(channel, (), Consumer(rx));

        // report_instance_properties(channel.clone()).await;
        // mark_ready_for_request();
        info!("Worker is ready...");

        let handle = reporter
            .reporting()
            .await
            // .with_graceful_shutdown(async move {
            //     sig.recv().await;
            //     info!("Shutdown signal received");
            // })
            .with_staus_handle(|status| {
                warn!(?status, "Collect failed");
            })
            .spawn();

        if let Err(err) = handle.await {
            error!(?err, "Tracer reporting failed");
        }
    };

    // TODO Do graceful shutdown, and wait 10s then force quit.
    select! {
        _ = sig_term.recv() => {}
        _ = sig_int.recv() => {}
        _ = fut => {}
    }

    info!("Start to shutdown skywalking grpc reporter");

    worker_exit();
}

#[tracing::instrument(skip_all)]
async fn connect(endpoint: Endpoint) -> Channel {
    let channel = loop {
        match endpoint.connect().await {
            Ok(channel) => break channel,
            Err(err) => {
                warn!(?err, "Connect to skywalking server failed, retry after 10s");
                sleep(Duration::from_secs(10)).await;
            }
        }
    };

    let uri = &*endpoint.uri().to_string();
    info!(uri, "Skywalking server connected");

    channel
}

struct Consumer(mpsc::Receiver<Result<CollectItem, Box<dyn Error + Send>>>);

#[async_trait]
impl ColletcItemConsume for Consumer {
    async fn consume(&mut self) -> Result<Option<CollectItem>, Box<dyn Error + Send>> {
        self.0
            .recv()
            .await
            .map(|result| result.map(Some))
            .unwrap_or(Ok(None))
    }

    async fn try_consume(&mut self) -> Result<Option<CollectItem>, Box<dyn Error + Send>> {
        self.0
            .try_recv()
            .map(|result| result.map(Some))
            .unwrap_or(Ok(None))
    }
}

fn worker_exit() {
    match Lazy::get(&SOCKET_FILE_PATH) {
        Some(socket_file) => {
            info!(socket_file, "Remove socket file");
            if let Err(err) = fs::remove_file(socket_file) {
                error!(?err, "Remove socket file failed");
            }
        }
        None => {
            warn!("Socket file not created");
        }
    }
}

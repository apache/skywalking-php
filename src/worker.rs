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

use crate::{channel, SKYWALKING_AGENT_SERVER_ADDR, SKYWALKING_AGENT_WORKER_THREADS};
use libc::fork;
use phper::ini::Ini;
use skywalking::reporter::grpc::GrpcReporter;
use std::{
    cmp::Ordering, num::NonZeroUsize, process::exit, thread::available_parallelism, time::Duration,
};
use tokio::{
    runtime::{self, Runtime},
    select,
    signal::unix::{signal, SignalKind},
    time::sleep,
};
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, error, info, warn};

pub fn init_worker() {
    let server_addr = Ini::get::<String>(SKYWALKING_AGENT_SERVER_ADDR).unwrap_or_default();
    let worker_threads = worker_threads();

    unsafe {
        // TODO Shutdown previous worker before fork if threre is a PHP-FPM reload
        // operation.
        // TODO Chagne the worker process name.

        let pid = fork();
        match pid.cmp(&0) {
            Ordering::Less => {
                error!("fork failed");
            }
            Ordering::Equal => {
                #[cfg(target_os = "linux")]
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);

                let rt = new_tokio_runtime(worker_threads);
                rt.block_on(start_worker(server_addr));
                exit(0);
            }
            Ordering::Greater => {}
        }
    }
}

fn worker_threads() -> usize {
    let worker_threads = Ini::get::<i64>(SKYWALKING_AGENT_WORKER_THREADS).unwrap_or(0);
    if worker_threads <= 0 {
        available_parallelism().map(NonZeroUsize::get).unwrap_or(1)
    } else {
        worker_threads as usize
    }
}

fn new_tokio_runtime(worker_threads: usize) -> Runtime {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .build()
        .unwrap()
}

async fn start_worker(server_addr: String) {
    debug!("Starting worker...");

    // Graceful shutdown signal, put it on the top of program.
    let mut sig = match signal(SignalKind::terminate()) {
        Ok(signal) => signal,
        Err(err) => {
            error!(?err, "Signal terminate failed");
            return;
        }
    };

    let fut = async move {
        let endpoint = match Endpoint::from_shared(server_addr) {
            Ok(endpoint) => endpoint,
            Err(err) => {
                error!(?err, "Create endpoint failed");
                return;
            }
        };
        let channel = connect(endpoint).await;

        let reporter = GrpcReporter::new_with_pc(channel, (), channel::Consumer);

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
        _ = sig.recv() => {}
        _ = fut => {}
    }
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

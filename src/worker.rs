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
    channel::{self, TxReporter},
    module::{
        AUTHENTICATION, ENABLE_TLS, HEARTBEAT_PERIOD, PROPERTIES_REPORT_PERIOD_FACTOR,
        SERVICE_INSTANCE, SERVICE_NAME, SOCKET_FILE_PATH, SSL_CERT_CHAIN_PATH, SSL_KEY_PATH,
        SSL_TRUSTED_CA_PATH,
    },
    util::change_permission,
    SKYWALKING_AGENT_SERVER_ADDR, SKYWALKING_AGENT_WORKER_THREADS,
};
use anyhow::anyhow;
use once_cell::sync::Lazy;
use phper::ini::ini_get;
use skywalking::{
    management::{instance::Properties, manager::Manager},
    reporter::{
        grpc::{CollectItemConsume, GrpcReporter},
        CollectItem,
    },
};
use std::{
    cmp::Ordering, error::Error, ffi::CStr, fs, io, marker::PhantomData, num::NonZeroUsize,
    process::exit, thread::available_parallelism, time::Duration,
};
use tokio::{
    net::UnixListener,
    runtime::{self, Runtime},
    select,
    signal::unix::{signal, SignalKind},
    sync::mpsc::{self, error::TrySendError},
    time::sleep,
};
use tonic::{
    async_trait,
    transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity},
};
use tracing::{debug, error, info, warn};

pub fn init_worker() {
    let server_addr = ini_get::<Option<&CStr>>(SKYWALKING_AGENT_SERVER_ADDR)
        .and_then(|s| s.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let worker_threads = worker_threads();

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

                // Run the worker in subprocess.
                let rt = new_tokio_runtime(worker_threads);
                match rt.block_on(start_worker(server_addr)) {
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

async fn start_worker(server_addr: String) -> anyhow::Result<()> {
    debug!("Starting worker...");

    // Ensure to cleanup resources when worker exits.
    let _guard = WorkerExitGuard::default();

    // Graceful shutdown signal, put it on the top of program.
    let mut sig_term = signal(SignalKind::terminate())?;
    let mut sig_int = signal(SignalKind::interrupt())?;

    let socket_file = &*SOCKET_FILE_PATH;

    let fut = async move {
        debug!(?socket_file, "Bind unix stream");
        let listener = UnixListener::bind(socket_file)?;
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

        let endpoint = create_endpoint(&server_addr).await?;
        let channel = connect(endpoint).await;

        report_properties_and_keep_alive(TxReporter(tx_));

        let mut reporter = GrpcReporter::new_with_pc(channel, (), Consumer(rx));

        if !AUTHENTICATION.is_empty() {
            reporter = reporter.with_authentication(&*AUTHENTICATION);
        }

        info!("Worker is ready...");

        let handle = reporter
            .reporting()
            .await
            .with_status_handle(|message, status| {
                warn!(?status, "Collect failed: {}", message);
            })
            .spawn();

        handle
            .await
            .map_err(|err| anyhow!("Tracer reporting failed: {:?}", err))?;

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

async fn create_endpoint(server_addr: &str) -> anyhow::Result<Endpoint> {
    let scheme = if *ENABLE_TLS { "https" } else { "http" };

    let url = format!("{}://{}", scheme, server_addr);
    debug!(url, "Create Endpoint");
    let mut endpoint = Endpoint::from_shared(url)?;

    if *ENABLE_TLS {
        let domain_name = server_addr.split(':').next().unwrap_or_default();
        debug!(domain_name, "Configure TLS domain");
        let mut tls = ClientTlsConfig::new().domain_name(domain_name);

        let ssl_trusted_ca_path = SSL_TRUSTED_CA_PATH.as_str();
        if !ssl_trusted_ca_path.is_empty() {
            debug!(ssl_trusted_ca_path, "Configure TLS CA");
            let ca_cert = tokio::fs::read(&*SSL_TRUSTED_CA_PATH).await?;
            let ca_cert = Certificate::from_pem(ca_cert);
            tls = tls.ca_certificate(ca_cert);
        }

        let ssl_key_path = SSL_KEY_PATH.as_str();
        let ssl_cert_chain_path = SSL_CERT_CHAIN_PATH.as_str();
        if !ssl_key_path.is_empty() && !ssl_cert_chain_path.is_empty() {
            debug!(ssl_trusted_ca_path, "Configure mTLS");
            let client_cert = tokio::fs::read(&*SSL_CERT_CHAIN_PATH).await?;
            let client_key = tokio::fs::read(&*SSL_KEY_PATH).await?;
            let client_identity = Identity::from_pem(client_cert, client_key);
            tls = tls.identity(client_identity);
        }

        endpoint = endpoint.tls_config(tls)?;
    }

    Ok(endpoint)
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

#[derive(Default)]
struct WorkerExitGuard(PhantomData<()>);

impl Drop for WorkerExitGuard {
    fn drop(&mut self) {
        match Lazy::get(&SOCKET_FILE_PATH) {
            Some(socket_file) => {
                info!(?socket_file, "Remove socket file");
                if let Err(err) = fs::remove_file(socket_file) {
                    error!(?err, "Remove socket file failed");
                }
            }
            None => {
                warn!("Socket file not created");
            }
        }
    }
}

fn report_properties_and_keep_alive(reporter: TxReporter) {
    let manager = Manager::new(&*SERVICE_NAME, &*SERVICE_INSTANCE, reporter);

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
        Duration::from_secs(*HEARTBEAT_PERIOD as u64),
        *PROPERTIES_REPORT_PERIOD_FACTOR as usize,
    );
}

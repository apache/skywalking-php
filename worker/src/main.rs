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

use clap::Parser;
use skywalking_php_worker::{
    WorkerConfiguration, new_tokio_runtime,
    reporter::{GrpcReporterConfiguration, KafkaReporterConfiguration, ReporterConfiguration},
    start_worker,
};
use std::{num::NonZeroUsize, path::PathBuf, thread::available_parallelism};
use tracing::log::LevelFilter;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path of socket file to listening
    #[arg(short, long)]
    socket_file_path: PathBuf,

    /// Count of worker threads, default is `nproc`
    #[arg(long)]
    worker_threads: Option<usize>,

    /// Log level, will be overwritten by env `RUST_LOG`
    #[arg(short, long, default_value = "INFO")]
    log_level: LevelFilter,

    /// Select reporter
    #[command(subcommand)]
    reporter: ReporterArgs,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
enum ReporterArgs {
    /// Report to Skywalking OAP via grpc protocol
    Grpc {
        /// skywalking server address
        #[arg(long)]
        server_addr: String,

        /// Skywalking agent authentication token
        #[arg(long)]
        authentication: Option<String>,

        /// Wether to enable tls for gPRC
        #[arg(long)]
        enable_tls: bool,

        /// The gRPC SSL trusted ca file
        #[arg(long, required_if_eq("enable_tls", "true"))]
        ssl_cert_chain_path: Option<String>,

        /// The private key file. Enable mTLS when ssl_key_path and
        /// ssl_cert_chain_path exist
        #[arg(long)]
        ssl_key_path: Option<String>,

        /// The certificate file. Enable mTLS when ssl_key_path and
        /// ssl_cert_chain_path exist
        #[arg(long)]
        ssl_trusted_ca_path: Option<String>,
    },
    /// Report to kafka
    Kafka {
        /// A list of host/port pairs to use for establishing the initial
        /// connection to the Kafka cluster. Only available when the
        /// reporter type is `kafka`
        #[arg(long)]
        kafka_bootstrap_servers: String,

        /// Configure Kafka Producer configuration in JSON format.
        /// Only available when the reporter type is `kafka`
        #[arg(long)]
        kafka_producer_config: Option<String>,
    },
}

impl From<ReporterArgs> for ReporterConfiguration {
    fn from(args: ReporterArgs) -> Self {
        match args {
            ReporterArgs::Grpc {
                server_addr,
                authentication,
                enable_tls,
                ssl_cert_chain_path,
                ssl_key_path,
                ssl_trusted_ca_path,
            } => ReporterConfiguration::Grpc(GrpcReporterConfiguration {
                server_addr,
                authentication: authentication.unwrap_or_default(),
                enable_tls,
                ssl_cert_chain_path: ssl_cert_chain_path.unwrap_or_default(),
                ssl_key_path: ssl_key_path.unwrap_or_default(),
                ssl_trusted_ca_path: ssl_trusted_ca_path.unwrap_or_default(),
            }),
            ReporterArgs::Kafka {
                kafka_bootstrap_servers,
                kafka_producer_config,
            } => ReporterConfiguration::Kafka(KafkaReporterConfiguration {
                kafka_bootstrap_servers,
                kafka_producer_config: kafka_producer_config.unwrap_or_default(),
            }),
        }
    }
}

fn init_logger(log_level: &LevelFilter) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "info,skywalking_agent={log_level},skywalking_php_worker={log_level}"
        ))
    });

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_ansi(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

fn worker_threads(worker_threads: Option<usize>) -> usize {
    worker_threads.unwrap_or_else(|| available_parallelism().map(NonZeroUsize::get).unwrap_or(1))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    init_logger(&args.log_level)?;

    let rt = new_tokio_runtime(worker_threads(args.worker_threads));

    rt.block_on(start_worker(WorkerConfiguration {
        socket_file_path: args.socket_file_path,
        heart_beat: None,
        reporter_config: args.reporter.into(),
    }))?;

    Ok(())
}

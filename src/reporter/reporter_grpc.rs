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
    AUTHENTICATION, ENABLE_TLS, SERVER_ADDR, SSL_CERT_CHAIN_PATH, SSL_KEY_PATH, SSL_TRUSTED_CA_PATH,
};
use anyhow::anyhow;
use skywalking::reporter::{grpc::GrpcReporter, CollectItemConsume, CollectItemProduce};
use std::time::Duration;
use tokio::time::sleep;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint, Identity};
use tracing::{debug, info, warn};

pub async fn run_reporter(
    producer: impl CollectItemProduce, consumer: impl CollectItemConsume,
) -> anyhow::Result<()> {
    let endpoint = create_endpoint(&SERVER_ADDR).await?;
    let channel = connect(endpoint).await;

    let mut reporter = GrpcReporter::new_with_pc(channel, producer, consumer);

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

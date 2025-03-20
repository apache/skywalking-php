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

#![cfg(feature = "kafka-reporter")]

use anyhow::{Context, bail};
use skywalking::reporter::{
    CollectItemConsume, CollectItemProduce,
    kafka::{KafkaReportBuilder, RDKafkaClientConfig},
};
use std::collections::HashMap;

pub struct KafkaReporterConfiguration {
    pub kafka_bootstrap_servers: String,
    pub kafka_producer_config: String,
}

pub async fn run_reporter(
    config: KafkaReporterConfiguration, producer: impl CollectItemProduce,
    consumer: impl CollectItemConsume,
) -> anyhow::Result<()> {
    let mut client_config = RDKafkaClientConfig::new();

    client_config.set("bootstrap.servers", config.kafka_bootstrap_servers);

    let config = serde_json::from_str::<HashMap<String, String>>(&config.kafka_producer_config)
        .context("parse kafka producer config failed")?;
    for (key, value) in config {
        client_config.set(key, value);
    }

    let (_, reporting) = KafkaReportBuilder::new_with_pc(client_config, producer, consumer)
        .build()
        .await?;
    let handle = reporting.spawn();
    if let Err(err) = handle.await {
        bail!("wait handle failed: {:?}", err);
    }

    Ok(())
}

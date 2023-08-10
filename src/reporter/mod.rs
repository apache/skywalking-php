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

mod reporter_grpc;
mod reporter_kafka;

use crate::module::REPORTER_TYPE;
use anyhow::bail;
use skywalking::reporter::{CollectItemConsume, CollectItemProduce};

pub async fn run_reporter(
    producer: impl CollectItemProduce, consumer: impl CollectItemConsume,
) -> anyhow::Result<()> {
    match REPORTER_TYPE.as_str() {
        "grpc" => reporter_grpc::run_reporter(producer, consumer).await,
        #[cfg(feature = "kafka-reporter")]
        "kafka" => reporter_kafka::run_reporter(producer, consumer).await,
        typ => bail!("unknown reporter type, {}", typ),
    }
}

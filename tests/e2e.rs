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

mod common;

use crate::common::{
    COLLECTOR_HTTP_ADDRESS, HTTP_CLIENT, PROXY_SERVER_1_ADDRESS, SWOOLE_SERVER_1_ADDRESS,
    SWOOLE_SERVER_2_ADDRESS,
};
use reqwest::{header::CONTENT_TYPE, RequestBuilder, StatusCode};
use std::{
    panic::{catch_unwind, resume_unwind},
    time::Duration,
};
use tokio::{fs::File, runtime::Handle, task, time::sleep};
use tracing::info;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn e2e() {
    let fixture = common::setup().await;

    // TODO Prefer to listen the server ready signal.
    sleep(Duration::from_secs(5)).await;

    let result = catch_unwind(|| {
        task::block_in_place(|| {
            Handle::current().block_on(run_e2e());
        });
    });

    common::teardown(fixture).await;

    if let Err(e) = result {
        resume_unwind(e);
    }
}

async fn run_e2e() {
    request_fpm_curl().await;
    request_fpm_curl_multi().await;
    request_fpm_pdo().await;
    request_fpm_predis().await;
    request_fpm_mysqli().await;
    request_fpm_memcached().await;
    request_fpm_redis().await;
    request_fpm_rabbitmq().await;
    request_swoole_curl().await;
    request_swoole_2_curl().await;
    request_swoole_2_pdo().await;
    request_swoole_2_mysqli().await;
    request_swoole_2_memcached().await;
    request_swoole_2_redis().await;
    request_swoole_2_predis().await;
    sleep(Duration::from_secs(3)).await;
    request_collector_validate().await;
}

async fn request_fpm_curl() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/curl.enter.php", PROXY_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_fpm_curl_multi() {
    request_common(
        HTTP_CLIENT.get(format!(
            "http://{}/curl-multi.enter.php",
            PROXY_SERVER_1_ADDRESS
        )),
        "ok",
    )
    .await;
}

async fn request_fpm_pdo() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/pdo.php", PROXY_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_fpm_mysqli() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/mysqli.php", PROXY_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_fpm_predis() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/predis.php", PROXY_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_fpm_memcached() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/memcached.php", PROXY_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_fpm_redis() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/redis.succ.php", PROXY_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;

    request_common(
        HTTP_CLIENT.get(format!("http://{}/redis.fail.php", PROXY_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_fpm_rabbitmq() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/rabbitmq.php", PROXY_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_swoole_curl() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/curl", SWOOLE_SERVER_1_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_swoole_2_curl() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/curl", SWOOLE_SERVER_2_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_swoole_2_pdo() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/pdo", SWOOLE_SERVER_2_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_swoole_2_mysqli() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/mysqli", SWOOLE_SERVER_2_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_swoole_2_memcached() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/memcached", SWOOLE_SERVER_2_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_swoole_2_redis() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/redis", SWOOLE_SERVER_2_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_swoole_2_predis() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/predis", SWOOLE_SERVER_2_ADDRESS)),
        "ok",
    )
    .await;
}

async fn request_collector_validate() {
    request_common(
        HTTP_CLIENT
            .post(format!("http://{}/dataValidate", COLLECTOR_HTTP_ADDRESS))
            .header(CONTENT_TYPE, "text/yaml")
            .body(
                File::open("./tests/data/expected_context.yaml")
                    .await
                    .unwrap(),
            ),
        "success",
    )
    .await;
}

async fn request_common(request_builder: RequestBuilder, actual_content: impl Into<String>) {
    let response = request_builder.send().await.unwrap();
    let status = response.status();
    let content = response.text().await.unwrap();
    info!(content, "response content");
    assert_eq!((status, content), (StatusCode::OK, actual_content.into()));
}

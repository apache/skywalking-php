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

use crate::common::{COLLECTOR_HTTP_ADDRESS, HTTP_CLIENT, PROXY_SERVER_1_ADDRESS};
use reqwest::{header::CONTENT_TYPE, RequestBuilder, StatusCode};
use std::{
    panic::{catch_unwind, resume_unwind},
    time::Duration,
};
use tokio::{fs::File, runtime::Handle, task, time::sleep};

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn e2e() {
    let fixture = common::setup().await;

    // TODO Prefer to listen the server ready signal.
    sleep(Duration::from_secs(3)).await;

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
    request_fpm_pdo().await;
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

async fn request_fpm_pdo() {
    request_common(
        HTTP_CLIENT.get(format!("http://{}/pdo.php", PROXY_SERVER_1_ADDRESS)),
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
    assert_eq!(
        (response.status(), response.text().await.unwrap()),
        (StatusCode::OK, actual_content.into())
    );
}

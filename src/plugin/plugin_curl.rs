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

use super::Plugin;
use crate::{
    component::COMPONENT_PHP_CURL_ID,
    context::RequestContext,
    execute::{validate_num_args, AfterExecuteHook, BeforeExecuteHook, Noop},
};
use anyhow::Context;
use phper::{
    arrays::{InsertKey, ZArray},
    functions::call,
    values::{ExecuteData, ZVal},
};
use skywalking::trace::{propagation::encoder::encode_propagation, span::Span};
use std::{cell::RefCell, collections::HashMap, os::raw::c_long};
use tracing::debug;
use url::Url;

static CURLOPT_HTTPHEADER: c_long = 10023;

/// Prevent calling `curl_setopt` inside this plugin sets headers, the hook of
/// `curl_setopt` is repeatedly called.
static SKY_CURLOPT_HTTPHEADER: c_long = 9923;

thread_local! {
    static CURL_HEADERS: RefCell<HashMap<i64, ZVal>> = Default::default();
}

#[derive(Default, Clone)]
pub struct CurlPlugin;

impl Plugin for CurlPlugin {
    #[inline]
    fn class_names(&self) -> Option<&'static [&'static str]> {
        None
    }

    #[inline]
    fn function_name_prefix(&self) -> Option<&'static str> {
        Some("curl_")
    }

    fn hook(
        &self, _class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)> {
        match function_name {
            "curl_setopt" => Some(self.hook_curl_setopt()),
            "curl_setopt_array" => Some(self.hook_curl_setopt_array()),
            "curl_exec" => Some(self.hook_curl_exec()),
            "curl_close" => Some(self.hook_curl_close()),
            _ => None,
        }
    }
}

impl CurlPlugin {
    fn hook_curl_setopt(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 3)?;

                let cid = Self::get_resource_id(execute_data)?;
                let options = execute_data.get_parameter(1).as_long();

                if options == Some(SKY_CURLOPT_HTTPHEADER) {
                    *execute_data.get_parameter(1) = CURLOPT_HTTPHEADER.into();
                } else if options == Some(CURLOPT_HTTPHEADER) {
                    let value = execute_data.get_parameter(2);
                    if value.get_type_info().is_array() {
                        CURL_HEADERS
                            .with(|headers| headers.borrow_mut().insert(cid, value.clone()));
                    }
                }

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn hook_curl_setopt_array(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 2)?;

                let cid = Self::get_resource_id(execute_data)?;

                if let Some(opts) = execute_data.get_parameter(1).as_z_arr() {
                    if let Some(value) = opts.get(CURLOPT_HTTPHEADER as u64) {
                        CURL_HEADERS
                            .with(|headers| headers.borrow_mut().insert(cid, value.clone()));
                    }
                }

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn hook_curl_exec(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|request_id, execute_data| {
                validate_num_args(execute_data, 1)?;

                let cid = Self::get_resource_id(execute_data)?;

                let ch = execute_data.get_parameter(0);
                let result = call("curl_getinfo", &mut [ch.clone()])?;
                let result = result.as_z_arr().context("result isn't array")?;

                let url = result
                    .get("url")
                    .context("Get url from curl_get_info result failed")?;
                let raw_url = url.as_z_str().context("url isn't string")?.to_str()?;
                let mut url = raw_url.to_string();

                if !url.contains("://") {
                    url.insert_str(0, "http://");
                }

                let url: Url = url.parse().context("parse url")?;
                if url.scheme() != "http" && url.scheme() != "https" {
                    return Ok(Box::new(()));
                }

                debug!("curl_getinfo get url: {}", &url);

                let host = match url.host_str() {
                    Some(host) => host,
                    None => return Ok(Box::new(())),
                };
                let port = match url.port() {
                    Some(port) => port,
                    None => match url.scheme() {
                        "http" => 80,
                        "https" => 443,
                        _ => 0,
                    },
                };
                let peer = &format!("{host}:{port}");

                let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    Ok(ctx.create_exit_span(url.path(), peer))
                })?;

                span.with_span_object_mut(|span| {
                    span.component_id = COMPONENT_PHP_CURL_ID;
                    span.add_tag("url", raw_url);
                });

                let sw_header = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    Ok(encode_propagation(ctx, url.path(), peer))
                })?;
                let mut val = CURL_HEADERS
                    .with(|headers| headers.borrow_mut().remove(&cid))
                    .unwrap_or_else(|| ZVal::from(ZArray::new()));
                if let Some(arr) = val.as_mut_z_arr() {
                    arr.insert(
                        InsertKey::NextIndex,
                        ZVal::from(format!("sw8: {}", sw_header)),
                    );
                    let ch = execute_data.get_parameter(0);
                    call(
                        "curl_setopt",
                        &mut [ch.clone(), ZVal::from(SKY_CURLOPT_HTTPHEADER), val],
                    )?;
                }

                Ok(Box::new(span))
            }),
            Box::new(move |_, span, execute_data, _| {
                let mut span = span.downcast::<Span>().unwrap();

                let ch = execute_data.get_parameter(0);
                let result = call("curl_getinfo", &mut [ch.clone()])?;
                let response = result.as_z_arr().context("response in not arr")?;
                let http_code = response
                    .get("http_code")
                    .and_then(|code| code.as_long())
                    .context("Call curl_getinfo, http_code is null")?;
                span.add_tag("status_code", &*http_code.to_string());
                if http_code == 0 {
                    let result = call("curl_error", &mut [ch.clone()])?;
                    let curl_error = result
                        .as_z_str()
                        .context("curl_error is not string")?
                        .to_str()?;
                    span.with_span_object_mut(|span| {
                        span.is_error = true;
                        span.add_log(vec![("CURL_ERROR", curl_error)]);
                    });
                } else if http_code >= 400 {
                    span.with_span_object_mut(|span| span.is_error = true);
                } else {
                    span.with_span_object_mut(|span| span.is_error = false);
                }

                Ok(())
            }),
        )
    }

    fn hook_curl_close(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 1)?;

                let cid = Self::get_resource_id(execute_data)?;

                CURL_HEADERS.with(|headers| headers.borrow_mut().remove(&cid));

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn get_resource_id(execute_data: &mut ExecuteData) -> anyhow::Result<i64> {
        // The `curl_init` return object since PHP8.
        let ch = execute_data.get_parameter(0);
        ch.as_z_res()
            .map(|res| res.handle())
            .or_else(|| ch.as_z_obj().map(|obj| obj.handle().into()))
            .context("Get resource id failed")
    }
}

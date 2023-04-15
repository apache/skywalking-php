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
    context::{RequestContext, SW_HEADER},
    execute::{validate_num_args, AfterExecuteHook, BeforeExecuteHook, Noop},
};
use anyhow::Context;
use phper::{
    arrays::{InsertKey, ZArray},
    functions::call,
    values::{ExecuteData, ZVal},
};
use skywalking::trace::span::Span;
use std::{cell::RefCell, collections::HashMap, os::raw::c_long};
use tracing::{debug, warn};
use url::Url;

const CURLM_OK: i64 = 0;

const CURLOPT_HTTPHEADER: c_long = 10023;

/// Prevent calling `curl_setopt` inside this plugin sets headers, the hook of
/// `curl_setopt` is repeatedly called.
const SKY_CURLOPT_HTTPHEADER: c_long = 9923;

thread_local! {
    static CURL_HEADERS: RefCell<HashMap<i64, ZVal>> = Default::default();
    static CURL_MULTI_INFO_MAP: RefCell<HashMap<i64, CurlMultiInfo>> = Default::default();
}

struct CurlInfo {
    cid: i64,
    raw_url: String,
    url: Url,
    peer: String,
    is_http: bool,
}

#[derive(Default)]
struct CurlMultiInfo {
    exec_spans: Option<Vec<(i64, Span)>>,
    curl_handles: HashMap<i64, ZVal>,
}

impl CurlMultiInfo {
    fn insert_curl_handle(&mut self, id: i64, handle: ZVal) {
        self.curl_handles.insert(id, handle);
    }

    fn remove_curl_handle(&mut self, id: i64) {
        self.curl_handles.remove(&id);
    }
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

            "curl_multi_add_handle" => Some(self.hook_curl_multi_add_handle()),
            "curl_multi_remove_handle" => Some(self.hook_curl_multi_remove_handle()),
            "curl_multi_exec" => Some(self.hook_curl_multi_exec()),
            "curl_multi_close" => Some(self.hook_curl_multi_close()),

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
                    *execute_data.get_mut_parameter(1) = CURLOPT_HTTPHEADER.into();
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

                let info = Self::get_curl_info(cid, ch.clone())?;

                let span = Self::create_exit_span(request_id, &info)?;

                if info.is_http {
                    Self::inject_sw_header(request_id, ch.clone(), &info)?;
                }

                Ok(Box::new(span))
            }),
            Box::new(move |_, span, execute_data, _| {
                let mut span = span.downcast::<Span>().unwrap();

                let ch = execute_data.get_parameter(0);
                Self::finish_exit_span(&mut span, ch)?;

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

    fn hook_curl_multi_add_handle(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 2)?;

                let multi_id = Self::get_resource_id(execute_data)?;
                let ch = execute_data.get_parameter(1);
                let cid = Self::get_handle_id(ch)?;

                CURL_MULTI_INFO_MAP.with(|map| {
                    map.borrow_mut()
                        .entry(multi_id)
                        .or_default()
                        .insert_curl_handle(cid, ch.clone());
                });

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn hook_curl_multi_remove_handle(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 2)?;

                let multi_id = Self::get_resource_id(execute_data)?;
                let ch = execute_data.get_parameter(1);
                let cid = Self::get_handle_id(ch)?;

                CURL_MULTI_INFO_MAP.with(|map| {
                    map.borrow_mut()
                        .entry(multi_id)
                        .or_default()
                        .remove_curl_handle(cid);
                });

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn hook_curl_multi_exec(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|request_id, execute_data| {
                validate_num_args(execute_data, 1)?;

                let multi_id = Self::get_resource_id(execute_data)?;

                let is_exec = CURL_MULTI_INFO_MAP.with(|map| {
                    let mut map = map.borrow_mut();
                    let Some(multi_info) = map.get_mut(&multi_id) else {
                        debug!(multi_id, "curl multi info is missing, maybe hasn't handles");
                        return Ok(false);
                    };

                    debug!(multi_id, "curl multi handles count: {}", multi_info.curl_handles.len());
                    if multi_info.curl_handles.is_empty() {
                        return Ok(false);
                    }
                    if multi_info.exec_spans.is_some() {
                        return Ok(true);
                    }

                    let mut curl_infos = Vec::with_capacity(multi_info.curl_handles.len());
                    for (cid, ch) in &multi_info.curl_handles {
                        curl_infos.push( (*cid, ch.clone(), Self::get_curl_info(*cid, ch.clone())?));
                    }
                    curl_infos.sort_by(|(_, _, i1), (_, _, i2)| i1.raw_url.cmp(&i2.raw_url));

                    let mut exec_spans = Vec::with_capacity(curl_infos.len());
                    for (cid, ch, info) in curl_infos {
                        let span = Self::create_exit_span(request_id, &info)?;

                        if info.is_http {
                            Self::inject_sw_header(request_id, ch, &info)?;
                        }

                        debug!(multi_id, operation_name = ?&span.span_object().operation_name, "create exit span");
                        exec_spans.push((cid, span));
                    }

                    // skywalking-rust can't create same level span at one time, so modify parent
                    // span id by hand.
                    if let [(_, head_span), tail_span @ ..] = exec_spans.as_mut_slice() {
                        let parent_span_id = head_span.span_object().parent_span_id;
                        for (_, span) in tail_span {
                            span.span_object_mut().parent_span_id = parent_span_id;
                        }
                    }

                    multi_info.exec_spans = Some(exec_spans);

                    Ok::<_, crate::Error>(true)
                })?;

                Ok(Box::new(is_exec))
            }),
            Box::new(move |_, is_exec, execute_data, return_value| {
                let is_exec = is_exec.downcast::<bool>().unwrap();
                if !*is_exec {
                    return Ok(());
                }

                if return_value.as_long() != Some(CURLM_OK) {
                    return Ok(());
                }

                let still_running = execute_data.get_parameter(1);
                if still_running
                    .as_z_ref()
                    .map(|r| r.val())
                    .and_then(|val| val.as_long())
                    != Some(0)
                {
                    return Ok(());
                }

                let multi_id = Self::get_resource_id(execute_data)?;
                debug!(multi_id, "curl multi exec has finished");

                CURL_MULTI_INFO_MAP.with(|map| {
                    let Some(mut info) = map.borrow_mut().remove(&multi_id) else {
                        warn!(multi_id, "curl multi info is missing after finished");
                        return Ok(());
                    };
                    let Some(mut spans) = info.exec_spans else {
                        warn!(multi_id, "curl multi spans is missing after finished");
                        return Ok(());
                    };

                    debug!(multi_id, "curl multi spans count: {}", spans.len());
                    loop {
                        let Some((cid, mut span)) = spans.pop() else { break };
                        let Some(ch) = info.curl_handles.remove(&cid) else  { continue };
                        Self::finish_exit_span(&mut span, &ch)?;
                    }
                    Ok::<_, crate::Error>(())
                })?;

                Ok(())
            }),
        )
    }

    fn hook_curl_multi_close(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 1)?;

                let multi_id = Self::get_resource_id(execute_data)?;

                CURL_MULTI_INFO_MAP.with(|map| map.borrow_mut().remove(&multi_id));

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn get_resource_id(execute_data: &mut ExecuteData) -> anyhow::Result<i64> {
        let ch = execute_data.get_parameter(0);
        Self::get_handle_id(ch)
    }

    fn get_handle_id(ch: &ZVal) -> anyhow::Result<i64> {
        // The `curl_init` return object since PHP8.
        ch.as_z_res()
            .map(|res| res.handle())
            .or_else(|| ch.as_z_obj().map(|obj| obj.handle().into()))
            .context("Get resource id failed")
    }

    fn get_curl_info(cid: i64, ch: ZVal) -> crate::Result<CurlInfo> {
        let result = call("curl_getinfo", &mut [ch])?;
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
        let is_http = ["http", "https"].contains(&url.scheme());

        debug!("curl_getinfo get url: {}", &url);

        let host = url.host_str().unwrap_or_default();
        let port = match url.port() {
            Some(port) => port,
            None => match url.scheme() {
                "http" => 80,
                "https" => 443,
                _ => 0,
            },
        };
        let peer = format!("{host}:{port}");

        Ok(CurlInfo {
            cid,
            raw_url: raw_url.to_string(),
            url,
            peer,
            is_http,
        })
    }

    fn inject_sw_header(request_id: Option<i64>, ch: ZVal, info: &CurlInfo) -> crate::Result<()> {
        let sw_header = RequestContext::try_get_sw_header(request_id)?;
        let mut val = CURL_HEADERS
            .with(|headers| headers.borrow_mut().remove(&info.cid))
            .unwrap_or_else(|| ZVal::from(ZArray::new()));
        if let Some(arr) = val.as_mut_z_arr() {
            arr.insert(
                InsertKey::NextIndex,
                ZVal::from(format!("{}: {}", SW_HEADER, sw_header)),
            );
            call(
                "curl_setopt",
                &mut [ch, ZVal::from(SKY_CURLOPT_HTTPHEADER), val],
            )?;
        }
        Ok(())
    }

    fn create_exit_span(request_id: Option<i64>, info: &CurlInfo) -> crate::Result<Span> {
        let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
            Ok(ctx.create_exit_span(info.url.path(), &info.peer))
        })?;

        let mut span_object = span.span_object_mut();
        span_object.component_id = COMPONENT_PHP_CURL_ID;
        span_object.add_tag("url", &info.raw_url);
        drop(span_object);

        Ok(span)
    }

    fn finish_exit_span(span: &mut Span, ch: &ZVal) -> crate::Result<()> {
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
            let mut span_object = span.span_object_mut();
            span_object.is_error = true;
            span_object.add_log(vec![("CURL_ERROR", curl_error)]);
        } else if http_code >= 400 {
            span.span_object_mut().is_error = true;
        } else {
            span.span_object_mut().is_error = false;
        }
        Ok(())
    }
}

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
    component::COMPONENT_PHP_ID,
    context::RequestContext,
    module::{is_enable, SKYWALKING_VERSION},
    util::{catch_unwind_result, get_sapi_module_name, z_val_to_string},
};
use anyhow::{anyhow, Context};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use phper::{arrays::ZArr, eg, pg, sg, sys, values::ZVal};
use skywalking::trace::{propagation::decoder::decode_propagation, span::AbstractSpan, tracer};
use std::{
    panic::AssertUnwindSafe,
    ptr::null_mut,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};
use tracing::{error, instrument, trace, warn};
use url::Url;

#[instrument(skip_all)]
pub fn init() {
    if !is_enable() {
        return;
    }
    if get_sapi_module_name().to_bytes() == b"fpm-fcgi" {
        if let Err(err) = catch_unwind_result(request_init_for_fpm) {
            error!(mode = "fpm", ?err, "request init failed");
        }
    }
}

#[instrument(skip_all)]
pub fn shutdown() {
    if !is_enable() {
        return;
    }
    if get_sapi_module_name().to_bytes() == b"fpm-fcgi" {
        if let Err(err) = catch_unwind_result(request_shutdown_for_fpm) {
            error!(mode = "fpm", ?err, "request shutdown failed");
        }
    }
}

fn request_init_for_fpm() -> crate::Result<()> {
    jit_initialization();

    let server = get_page_request_server()?;

    let header = get_page_request_header(server);
    let url = get_page_request_url(server)?;
    let method = get_page_request_method(server);

    create_request_context(None, header.as_deref(), &method, &url)
}

fn request_shutdown_for_fpm() -> crate::Result<()> {
    let status_code = unsafe { sg!(sapi_headers).http_response_code };

    finish_request_context(None, status_code)
}

#[allow(clippy::useless_conversion)]
fn jit_initialization() {
    unsafe {
        let jit_initialization: u8 = pg!(auto_globals_jit).into();
        if jit_initialization != 0 {
            let mut server = "_SERVER".to_string();
            sys::zend_is_auto_global_str(server.as_mut_ptr().cast(), server.len());
        }
    }
}

fn get_page_request_header(server: &ZArr) -> Option<String> {
    if *SKYWALKING_VERSION >= 8 {
        server
            .get("HTTP_SW8")
            .and_then(|sw| sw.as_z_str())
            .and_then(|zs| zs.to_str().ok())
            .map(|s| s.to_string())
    } else {
        None
    }
}

fn get_page_request_url(server: &ZArr) -> crate::Result<Url> {
    let scheme = if [Some("1"), Some("on")]
        .contains(&server.get("HTTPS").and_then(z_val_to_string).as_deref())
    {
        "https"
    } else {
        "http"
    };

    let addr = server
        .get("HTTP_HOST")
        .and_then(z_val_to_string)
        .or_else(|| {
            server
                .get("SERVER_PORT")
                .and_then(z_val_to_string)
                .map(|port| format!("localhost:{}", port))
        })
        .unwrap_or_else(|| "unknown:0".to_string());

    let uri = server
        .get("REQUEST_URI")
        .and_then(z_val_to_string)
        .or_else(|| server.get("PHP_SELF").and_then(z_val_to_string))
        .or_else(|| server.get("SCRIPT_NAME").and_then(z_val_to_string))
        .unwrap_or_else(|| "/unknown".to_string());

    Ok(Url::parse(&format!("{}://{}{}", scheme, addr, uri))?)
}

fn get_page_request_method(server: &ZArr) -> String {
    server
        .get("REQUEST_METHOD")
        .and_then(z_val_to_string)
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

fn get_page_request_server<'a>() -> anyhow::Result<&'a ZArr> {
    unsafe {
        let symbol_table = ZArr::from_mut_ptr(&mut eg!(symbol_table));
        let carrier = symbol_table
            .get("_SERVER")
            .and_then(|carrier| carrier.as_z_arr())
            .context("$_SERVER is null")?;
        Ok(carrier)
    }
}

pub const HACK_SWOOLE_ON_REQUEST_FUNCTION_NAME: &str =
    "skywalking_hack_swoole_on_request_please_do_not_use";

/// Hold the response fd and status code kvs, because I dont't found that
/// response has the status field, so I hook the response.status method, maybe
/// there is a better way?
pub static SWOOLE_RESPONSE_STATUS_MAP: Lazy<DashMap<i64, i32>> = Lazy::new(DashMap::new);

pub static ORI_SWOOLE_ON_REQUEST: AtomicPtr<sys::zval> = AtomicPtr::new(null_mut());

pub static IS_SWOOLE: AtomicBool = AtomicBool::new(false);

/// The function is used by swoole plugin, to surround the callback of on
/// request.
pub fn skywalking_hack_swoole_on_request(args: &mut [ZVal]) -> phper::Result<ZVal> {
    let f = ORI_SWOOLE_ON_REQUEST.load(Ordering::Relaxed);
    if f.is_null() {
        error!("Origin swoole on request handler is null");
        return Ok(ZVal::from(()));
    }
    let f = unsafe { ZVal::from_mut_ptr(f) };

    let result = catch_unwind_result(AssertUnwindSafe(|| request_init_for_swoole(&mut args[0])));
    if let Err(err) = &result {
        error!(mode = "swoole", ?err, "request init failed");
    }

    let return_value = f.call(&mut *args);
    if let Err(err) = &return_value {
        error!(
            mode = "swoole",
            ?err,
            "Something wrong when call the origin on-request handler"
        );
    }

    if result.is_ok() {
        if let Err(err) = catch_unwind_result(AssertUnwindSafe(move || {
            request_shutdown_for_swoole(&mut args[1])
        })) {
            error!(mode = "swoole", ?err, "request shutdown failed");
        }
    }

    return_value
}

fn request_init_for_swoole(request: &mut ZVal) -> crate::Result<()> {
    let request = request.as_z_obj().context("swoole request isn't object")?;

    let fd = request
        .get_property("fd")
        .as_long()
        .context("swoole request fd not exists")?;

    let headers = request
        .get_property("header")
        .as_z_arr()
        .context("swoole request header not exists")?;

    let header = get_swoole_request_header(headers);

    let server = request
        .get_property("server")
        .as_z_arr()
        .context("swoole request server not exists")?;

    let method = get_swoole_request_method(server);
    let url = get_swoole_request_url(server, headers)?;

    create_request_context(Some(fd), header.as_deref(), &method, &url)
}

fn request_shutdown_for_swoole(response: &mut ZVal) -> crate::Result<()> {
    let response = response
        .as_mut_z_obj()
        .context("swoole response isn't object")?;

    let fd = response
        .get_mut_property("fd")
        .as_long()
        .context("swoole request fd not exists")?;

    finish_request_context(
        Some(fd),
        SWOOLE_RESPONSE_STATUS_MAP
            .remove(&fd)
            .map(|(_, status)| status)
            .unwrap_or(200),
    )
}

fn get_swoole_request_header(header: &ZArr) -> Option<String> {
    if *SKYWALKING_VERSION >= 8 {
        header
            .get("sw8")
            .and_then(|sw| sw.as_z_str())
            .and_then(|zs| zs.to_str().ok())
            .map(|s| s.to_string())
    } else {
        None
    }
}

fn get_swoole_request_url(server: &ZArr, headers: &ZArr) -> crate::Result<Url> {
    let addr = headers
        .get("host")
        .and_then(z_val_to_string)
        .or_else(|| {
            server
                .get("server_port")
                .and_then(z_val_to_string)
                .map(|port| format!("localhost:{}", port))
        })
        .unwrap_or_else(|| "unknown:0".to_string());

    let uri = server
        .get("request_uri")
        .and_then(z_val_to_string)
        .unwrap_or_else(|| "/unknown".to_string());

    let query = server
        .get("query_string")
        .and_then(z_val_to_string)
        .map(|s| format!("?{}", s))
        .unwrap_or_default();

    Ok(Url::parse(&format!("http://{}{}{}", addr, uri, query))?)
}

fn get_swoole_request_method(server: &ZArr) -> String {
    server
        .get("request_method")
        .and_then(z_val_to_string)
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

fn create_request_context(
    request_id: Option<i64>, header: Option<&str>, method: &str, url: &Url,
) -> crate::Result<()> {
    let propagation = header
        .map(decode_propagation)
        .transpose()
        .map_err(|e| anyhow!("decode propagation failed: {}", e))?;

    trace!("Propagation: {:?}", &propagation);

    let mut ctx = tracer::create_trace_context();

    let operation_name = format!("{}:{}", method, url.path());
    let mut span = match propagation {
        Some(propagation) => ctx.create_entry_span_with_propagation(&operation_name, &propagation),
        None => ctx.create_entry_span(&operation_name),
    };

    let mut span_object = span.span_object_mut();
    span_object.component_id = COMPONENT_PHP_ID;
    span_object.add_tag("url", url.to_string());
    span_object.add_tag("http.method", method);

    RequestContext::set_global(
        request_id,
        RequestContext {
            tracing_context: ctx,
            entry_span: span,
        },
    );

    Ok(())
}

fn finish_request_context(request_id: Option<i64>, status_code: i32) -> crate::Result<()> {
    let RequestContext {
        tracing_context,
        mut entry_span,
    } = RequestContext::remove_global(request_id).context("request context not exists")?;

    entry_span.add_tag("http.status_code", &status_code.to_string());
    if status_code >= 400 {
        entry_span.span_object_mut().is_error = true;
    }

    drop(entry_span);
    drop(tracing_context);

    Ok(())
}

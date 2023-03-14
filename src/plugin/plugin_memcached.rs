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

use std::{any::Any, collections::HashMap};

use super::Plugin;
use crate::{
    component::COMPONENT_PHP_MEMCACHED_ID,
    context::RequestContext,
    execute::{get_this_mut, AfterExecuteHook, BeforeExecuteHook},
    tag::{CacheOp, TAG_CACHE_CMD, TAG_CACHE_KEY, TAG_CACHE_OP, TAG_CACHE_TYPE},
};
use anyhow::Context;
use once_cell::sync::Lazy;
use phper::{
    objects::ZObj,
    values::{ExecuteData, ZVal},
};
use skywalking::{skywalking_proto::v3::SpanLayer, trace::span::Span};
use tracing::{debug, instrument, warn};

/// The method parameters is empty.
static MEMCACHE_EMPTY_METHOD_MAPPING: Lazy<HashMap<&str, TagInfo<'static>>> = Lazy::new(|| {
    [
        ("getallkeys", TagInfo::new(None, None)),
        ("getstats", TagInfo::new(Some("stats"), None)),
        ("flush", TagInfo::new(None, None)),
        ("getversion", TagInfo::new(Some("version"), None)),
    ]
    .into_iter()
    .collect()
});

/// The method first parameter is key.
static MEMCACHE_KEY_METHOD_MAPPING: Lazy<HashMap<&str, TagInfo<'static>>> = Lazy::new(|| {
    [
        ("set", TagInfo::new(Some("set"), Some(CacheOp::Write))),
        ("setmulti", TagInfo::new(Some("set"), Some(CacheOp::Write))),
        ("add", TagInfo::new(Some("add"), Some(CacheOp::Write))),
        (
            "replace",
            TagInfo::new(Some("replace"), Some(CacheOp::Write)),
        ),
        ("append", TagInfo::new(Some("append"), Some(CacheOp::Write))),
        (
            "prepend",
            TagInfo::new(Some("prepend"), Some(CacheOp::Write)),
        ),
        ("get", TagInfo::new(Some("get"), Some(CacheOp::Read))),
        ("getmulti", TagInfo::new(Some("get"), Some(CacheOp::Read))),
        ("delete", TagInfo::new(Some("delete"), Some(CacheOp::Write))),
        (
            "deletemulti",
            TagInfo::new(Some("deleteMulti"), Some(CacheOp::Write)),
        ),
        (
            "increment",
            TagInfo::new(Some("increment"), Some(CacheOp::Write)),
        ),
        (
            "decrement",
            TagInfo::new(Some("decrement"), Some(CacheOp::Write)),
        ),
    ]
    .into_iter()
    .collect()
});

/// The method first parameter is server key and second parameter is key.
static MEMCACHE_SERVER_KEY_METHOD_MAPPING: Lazy<HashMap<&str, TagInfo<'static>>> =
    Lazy::new(|| {
        [
            ("setByKey", TagInfo::new(Some("set"), Some(CacheOp::Write))),
            (
                "setMultiByKey",
                TagInfo::new(Some("set"), Some(CacheOp::Write)),
            ),
            ("addByKey", TagInfo::new(Some("add"), Some(CacheOp::Write))),
            (
                "replaceByKey",
                TagInfo::new(Some("replace"), Some(CacheOp::Write)),
            ),
            (
                "appendByKey",
                TagInfo::new(Some("append"), Some(CacheOp::Write)),
            ),
            (
                "prependByKey",
                TagInfo::new(Some("prepend"), Some(CacheOp::Write)),
            ),
            ("getByKey", TagInfo::new(Some("get"), Some(CacheOp::Read))),
            (
                "getMultiByKey",
                TagInfo::new(Some("get"), Some(CacheOp::Read)),
            ),
            (
                "deleteByKey",
                TagInfo::new(Some("delete"), Some(CacheOp::Write)),
            ),
            (
                "deleteMultiByKey",
                TagInfo::new(Some("deleteMulti"), Some(CacheOp::Write)),
            ),
            (
                "incrementByKey",
                TagInfo::new(Some("increment"), Some(CacheOp::Write)),
            ),
            (
                "decrementByKey",
                TagInfo::new(Some("decrement"), Some(CacheOp::Write)),
            ),
        ]
        .into_iter()
        .collect()
    });

struct TagInfo<'a> {
    cmd: Option<&'a str>,
    op: Option<CacheOp>,
}

impl<'a> TagInfo<'a> {
    fn new(cmd: Option<&'a str>, op: Option<CacheOp>) -> Self {
        Self { cmd, op }
    }
}

#[derive(Default, Clone)]
pub struct MemcachedPlugin;

impl Plugin for MemcachedPlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["Memcached"])
    }

    fn function_name_prefix(&self) -> Option<&'static str> {
        None
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(
        Box<crate::execute::BeforeExecuteHook>,
        Box<crate::execute::AfterExecuteHook>,
    )> {
        match (class_name, function_name) {
            (Some(class_name @ "Memcached"), f)
                if MEMCACHE_EMPTY_METHOD_MAPPING.contains_key(&*f.to_ascii_lowercase()) =>
            {
                Some(self.hook_memcached_empty_methods(class_name, function_name))
            }
            (Some(class_name @ "Memcached"), f)
                if MEMCACHE_KEY_METHOD_MAPPING.contains_key(&*f.to_ascii_lowercase()) =>
            {
                Some(self.hook_memcached_key_methods(class_name, function_name))
            }
            (Some(class_name @ "Memcached"), f)
                if MEMCACHE_SERVER_KEY_METHOD_MAPPING.contains_key(&*f.to_ascii_lowercase()) =>
            {
                Some(self.hook_memcached_server_key_methods(class_name, function_name))
            }
            _ => None,
        }
    }
}

impl MemcachedPlugin {
    #[instrument(skip_all)]
    fn hook_memcached_empty_methods(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, _| {
                let tag_info = MEMCACHE_EMPTY_METHOD_MAPPING
                    .get(&*function_name.to_ascii_lowercase())
                    .unwrap();

                let span =
                    create_exit_span(request_id, &class_name, &function_name, "", tag_info, None)?;

                Ok(Box::new(span))
            }),
            Box::new(after_hook),
        )
    }

    #[instrument(skip_all)]
    fn hook_memcached_key_methods(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let key = {
                    let key = execute_data.get_parameter(0);
                    if key.get_type_info().is_string() {
                        Some(key.clone())
                    } else {
                        // The `*Multi` methods will failed here.
                        warn!("The argument key of {} isn't string", &function_name);
                        None
                    }
                };

                let key_str = key
                    .as_ref()
                    .and_then(|key| key.as_z_str())
                    .and_then(|key| key.to_str().ok())
                    .map(ToOwned::to_owned);

                let this = get_this_mut(execute_data)?;

                let peer = key.map(|key| get_peer(this, key)).unwrap_or_default();

                debug!(peer, "Get memcached peer");

                let tag_info = MEMCACHE_KEY_METHOD_MAPPING
                    .get(&*function_name.to_ascii_lowercase())
                    .unwrap();

                let span = create_exit_span(
                    request_id,
                    &class_name,
                    &function_name,
                    &peer,
                    tag_info,
                    key_str.as_deref(),
                )?;

                Ok(Box::new(span))
            }),
            Box::new(after_hook),
        )
    }

    #[instrument(skip_all)]
    fn hook_memcached_server_key_methods(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let server_key = {
                    let server_key = execute_data.get_parameter(0);
                    if server_key.get_type_info().is_string() {
                        Some(server_key.clone())
                    } else {
                        // The `*Multi` methods will failed here.
                        warn!(function_name, "The argument server_key isn't string");
                        None
                    }
                };

                let key = execute_data
                    .get_parameter(1)
                    .as_z_str()
                    .and_then(|key| key.to_str().ok())
                    .map(ToOwned::to_owned);

                let this = get_this_mut(execute_data)?;

                let peer = server_key
                    .map(|server_key| get_peer(this, server_key))
                    .unwrap_or_default();

                debug!(peer, "Get memcached peer");

                let tag_info = MEMCACHE_SERVER_KEY_METHOD_MAPPING
                    .get(&*function_name.to_ascii_lowercase())
                    .unwrap();

                let span = create_exit_span(
                    request_id,
                    &class_name,
                    &function_name,
                    &peer,
                    tag_info,
                    key.as_deref(),
                )?;

                Ok(Box::new(span))
            }),
            Box::new(after_hook),
        )
    }
}

#[instrument(skip_all)]
fn after_hook(
    _: Option<i64>, span: Box<dyn Any>, execute_data: &mut ExecuteData, return_value: &mut ZVal,
) -> crate::Result<()> {
    let mut span = span.downcast::<Span>().expect("Downcast to Span failed");
    if let Some(b) = return_value.as_bool() {
        if !b {
            span.span_object_mut().is_error = true;

            let this = get_this_mut(execute_data)?;
            let code = this.call(&"getResultCode".to_ascii_lowercase(), [])?;
            let code = code.as_long().context("ResultCode isn't int")?;
            debug!(code, "get memcached result code");

            if code != 0 {
                let message = this.call(&"getResultMessage".to_ascii_lowercase(), [])?;
                let message = message
                    .as_z_str()
                    .context("ResultMessage isn't string")?
                    .to_str()?;
                debug!(message, "get memcached result message");

                span.add_log([
                    ("ResultCode", code.to_string()),
                    ("ResultMessage", message.to_owned()),
                ]);
            }
        }
    }
    Ok(())
}

fn create_exit_span<'a>(
    request_id: Option<i64>, class_name: &str, function_name: &str, remote_peer: &str,
    tag_info: &TagInfo<'a>, key: Option<&str>,
) -> anyhow::Result<Span> {
    RequestContext::try_with_global_ctx(request_id, |ctx| {
        let mut span =
            ctx.create_exit_span(&format!("{}->{}", class_name, function_name), remote_peer);

        let mut span_object = span.span_object_mut();
        span_object.set_span_layer(SpanLayer::Cache);
        span_object.component_id = COMPONENT_PHP_MEMCACHED_ID;
        span_object.add_tag(TAG_CACHE_TYPE, "memcache");
        if let Some(cmd) = tag_info.cmd {
            span_object.add_tag(TAG_CACHE_CMD, cmd);
        }
        if let Some(op) = &tag_info.op {
            span_object.add_tag(TAG_CACHE_OP, op.to_string());
        };
        if let Some(key) = key {
            span_object.add_tag(TAG_CACHE_KEY, key)
        }
        drop(span_object);

        Ok(span)
    })
}

fn get_peer(this: &mut ZObj, key: ZVal) -> String {
    let f = || {
        let info = this.call(&"getServerByKey".to_ascii_lowercase(), [key])?;
        let info = info.as_z_arr().context("Server isn't array")?;
        let host = info
            .get("host")
            .context("Server host not exists")?
            .as_z_str()
            .context("Server host isn't string")?
            .to_str()?;
        let port = info
            .get("port")
            .context("Server port not exists")?
            .as_long()
            .context("Server port isn't long")?;
        Ok::<_, crate::Error>(format!("{}:{}", host, port))
    };
    f().unwrap_or_else(|err| {
        warn!(?err, "Get peer failed");
        "".to_owned()
    })
}

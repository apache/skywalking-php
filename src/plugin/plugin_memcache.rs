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

use super::{log_exception, style::ApiStyle, Plugin};
use crate::{
    component::COMPONENT_PHP_MEMCACHED_ID,
    context::RequestContext,
    execute::{AfterExecuteHook, BeforeExecuteHook, Noop},
    tag::{CacheOp, TAG_CACHE_CMD, TAG_CACHE_KEY, TAG_CACHE_OP, TAG_CACHE_TYPE},
};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use phper::{
    arrays::IterKey,
    objects::ZObj,
    values::{ExecuteData, ZVal},
};
use skywalking::{
    proto::v3::SpanLayer,
    trace::span::{HandleSpanObject, Span},
};
use std::{any::Any, collections::HashMap};
use tracing::{debug, error, instrument, warn};

static PEER_MAP: Lazy<DashMap<u32, String>> = Lazy::new(Default::default);

/// The method parameters is empty.
static MEMCACHE_EMPTY_METHOD_MAPPING: Lazy<HashMap<&str, TagInfo<'static>>> =
    Lazy::new(|| [("flush", TagInfo::new(None, None))].into_iter().collect());

/// The method first parameter is key.
static MEMCACHE_KEY_METHOD_MAPPING: Lazy<HashMap<&str, TagInfo<'static>>> = Lazy::new(|| {
    [
        ("set", TagInfo::new(Some("set"), Some(CacheOp::Write))),
        ("add", TagInfo::new(Some("add"), Some(CacheOp::Write))),
        (
            "replace",
            TagInfo::new(Some("replace"), Some(CacheOp::Write)),
        ),
        ("get", TagInfo::new(Some("get"), Some(CacheOp::Read))),
        ("delete", TagInfo::new(Some("delete"), Some(CacheOp::Write))),
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

struct TagInfo<'a> {
    cmd: Option<&'a str>,
    op: Option<CacheOp>,
}

impl<'a> TagInfo<'a> {
    #[inline]
    fn new(cmd: Option<&'a str>, op: Option<CacheOp>) -> Self {
        Self { cmd, op }
    }
}

#[derive(Default, Clone)]
pub struct MemcachePlugin;

impl Plugin for MemcachePlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["Memcache", "MemcachePool"])
    }

    fn function_name_prefix(&self) -> Option<&'static str> {
        Some("memcache_")
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(
        Box<crate::execute::BeforeExecuteHook>,
        Box<crate::execute::AfterExecuteHook>,
    )> {
        let lowercase_function_name = function_name.to_ascii_lowercase();
        let function_name = function_name.to_owned();

        match (class_name, &*lowercase_function_name) {
            (Some("Memcache" | "MemcachePool"), "connect" | "addserver" | "close") => {
                Some(self.hook_memcache_server(
                    class_name.map(ToOwned::to_owned),
                    function_name,
                    ApiStyle::OO,
                ))
            }
            (None, "memcache_add_server" | "memcache_close") => {
                Some(self.hook_memcache_server(None, function_name, ApiStyle::Procedural))
            }
            (Some("Memcache" | "MemcachePool"), f)
                if MEMCACHE_EMPTY_METHOD_MAPPING.contains_key(f) =>
            {
                Some(self.hook_memcache_empty_methods(
                    class_name.map(ToOwned::to_owned),
                    function_name,
                    ApiStyle::OO,
                ))
            }
            (None, f) if MEMCACHE_EMPTY_METHOD_MAPPING.contains_key(&f["memcache_".len()..]) => {
                Some(self.hook_memcache_empty_methods(None, function_name, ApiStyle::Procedural))
            }
            (Some("Memcache" | "MemcachePool"), f)
                if MEMCACHE_KEY_METHOD_MAPPING.contains_key(f) =>
            {
                Some(self.hook_memcache_key_methods(
                    class_name.map(ToOwned::to_owned),
                    function_name,
                    ApiStyle::OO,
                ))
            }
            (None, f) if MEMCACHE_KEY_METHOD_MAPPING.contains_key(&f["memcache_".len()..]) => {
                Some(self.hook_memcache_key_methods(None, function_name, ApiStyle::Procedural))
            }
            _ => None,
        }
    }
}

impl MemcachePlugin {
    fn hook_memcache_server(
        &self, class_name: Option<String>, function_name: String, style: ApiStyle,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(move |_, execute_data| {
                let this = style.get_this_mut(execute_data)?;
                let handle = this.handle();
                PEER_MAP.remove(&handle);

                debug!(
                    handle,
                    ?class_name,
                    function_name,
                    "remove peers cache when server added"
                );

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    #[instrument(skip_all)]
    fn hook_memcache_empty_methods(
        &self, class_name: Option<String>, function_name: String, style: ApiStyle,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(move |request_id, execute_data| {
                let tag_info = MEMCACHE_EMPTY_METHOD_MAPPING
                    .get(&*get_tag_key(class_name.as_deref(), &function_name))
                    .unwrap();

                let this = style.get_this_mut(execute_data)?;
                let peer = get_peer(this);

                let span = create_exit_span(
                    style,
                    request_id,
                    class_name.as_deref(),
                    &function_name,
                    &peer,
                    tag_info,
                    None,
                )?;

                Ok(Box::new(span))
            }),
            Box::new(after_hook),
        )
    }

    #[instrument(skip_all)]
    fn hook_memcache_key_methods(
        &self, class_name: Option<String>, function_name: String, style: ApiStyle,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(move |request_id, execute_data| {
                let tag_info = MEMCACHE_KEY_METHOD_MAPPING
                    .get(&*get_tag_key(class_name.as_deref(), &function_name))
                    .unwrap();

                let key = style
                    .get_mut_parameter(execute_data, 0)
                    .as_z_str()
                    .and_then(|s| s.to_str().ok())
                    .map(ToOwned::to_owned)
                    .unwrap_or_default();

                let this = style.get_this_mut(execute_data)?;
                let peer = get_peer(this);

                let span = create_exit_span(
                    style,
                    request_id,
                    class_name.as_deref(),
                    &function_name,
                    &peer,
                    tag_info,
                    Some(&key),
                )?;

                Ok(Box::new(span))
            }),
            Box::new(after_hook),
        )
    }
}

#[instrument(skip_all)]
fn after_hook(
    _: Option<i64>, span: Box<dyn Any>, _: &mut ExecuteData, return_value: &mut ZVal,
) -> crate::Result<()> {
    let mut span = span.downcast::<Span>().expect("Downcast to Span failed");

    if let Some(b) = return_value.as_bool() {
        if !b {
            span.span_object_mut().is_error = true;
        }
    }

    log_exception(&mut *span);

    Ok(())
}

fn create_exit_span(
    style: ApiStyle, request_id: Option<i64>, class_name: Option<&str>, function_name: &str,
    remote_peer: &str, tag_info: &TagInfo<'_>, key: Option<&str>,
) -> anyhow::Result<Span> {
    RequestContext::try_with_global_ctx(request_id, |ctx| {
        let mut span = ctx.create_exit_span(
            &style.generate_peer_name(class_name, function_name),
            remote_peer,
        );

        let span_object = span.span_object_mut();
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

        Ok(span)
    })
}

fn get_peer(this: &mut ZObj) -> String {
    let handle = this.handle();

    PEER_MAP
        .entry(handle)
        .or_insert_with(|| {
            debug!(
                handle,
                "start to call {:?}::getExtendedStats method",
                this.get_class().get_name()
            );
            let stats = match this.call("getExtendedStats", []) {
                Ok(stats) => stats,
                Err(err) => {
                    error!(
                        ?err,
                        "call {:?}::getExtendedStats method failed",
                        this.get_class().get_name()
                    );
                    return "".to_owned();
                }
            };

            stats
                .as_z_arr()
                .map(|arr| {
                    arr.iter()
                        .map(|(key, _)| match key {
                            IterKey::Index(i) => i.to_string(),
                            IterKey::ZStr(s) => s.to_str().unwrap_or_default().to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default()
        })
        .value()
        .clone()
}

fn get_tag_key(class_name: Option<&str>, function_name: &str) -> String {
    match class_name {
        Some(_) => function_name.to_ascii_lowercase(),
        None => function_name["memcache_".len()..].to_string(),
    }
}

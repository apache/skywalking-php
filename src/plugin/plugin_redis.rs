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

use std::{any::Any, collections::HashSet};

use super::Plugin;
use crate::{
    component::COMPONENT_PHP_REDIS_ID,
    context::RequestContext,
    execute::{get_this_mut, AfterExecuteHook, BeforeExecuteHook, Noop},
    util::json_encode_values,
};
use anyhow::Context;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use phper::{
    eg,
    objects::ZObj,
    sys,
    values::{ExecuteData, ZVal},
};
use skywalking::{skywalking_proto::v3::SpanLayer, trace::span::Span};
use tracing::{debug, warn};

static PEER_MAP: Lazy<DashMap<u32, Peer>> = Lazy::new(Default::default);

static FREE_MAP: Lazy<DashMap<u32, sys::zend_object_free_obj_t>> = Lazy::new(Default::default);

static REDIS_COMMANDS: Lazy<HashSet<String>> = Lazy::new(|| {
    [
        "ping",
        "echo",
        "append",
        "bitCount",
        "bitOp",
        "decr",
        "decrBy",
        "get",
        "getBit",
        "getRange",
        "getSet",
        "incr",
        "incrBy",
        "incrByFloat",
        "mGet",
        "getMultiple",
        "mSet",
        "mSetNX",
        "set",
        "setBit",
        "setEx",
        "pSetEx",
        "setNx",
        "setRange",
        "strLen",
        "del",
        "delete",
        "unlink",
        "dump",
        "exists",
        "expire",
        "setTimeout",
        "pexpire",
        "expireAt",
        "pexpireAt",
        "keys",
        "getKeys",
        "scan",
        "migrate",
        "move",
        "object",
        "persist",
        "randomKey",
        "rename",
        "renameKey",
        "renameNx",
        "type",
        "sort",
        "ttl",
        "pttl",
        "restore",
        "hDel",
        "hExists",
        "hGet",
        "hGetAll",
        "hIncrBy",
        "hIncrByFloat",
        "hKeys",
        "hLen",
        "hMGet",
        "hMSet",
        "hSet",
        "hSetNx",
        "hVals",
        "hScan",
        "hStrLen",
        "blPop",
        "brPop",
        "bRPopLPush",
        "lIndex",
        "lGet",
        "lInsert",
        "lLen",
        "lSize",
        "lPop",
        "lPush",
        "lPushx",
        "lRange",
        "lGetRange",
        "lRem",
        "lRemove",
        "lSet",
        "lTrim",
        "listTrim",
        "rPop",
        "rPopLPush",
        "rPush",
        "rPushX",
        "sAdd",
        "sCard",
        "sSize",
        "sDiff",
        "sDiffStore",
        "sInter",
        "sInterStore",
        "sIsMember",
        "sContains",
        "sMembers",
        "sGetMembers",
        "sMove",
        "sPop",
        "sRandMember",
        "sRem",
        "sRemove",
        "sUnion",
        "sUnionStore",
        "sScan",
        "bzPop",
        "zAdd",
        "zCard",
        "zSize",
        "zCount",
        "zIncrBy",
        "zinterstore",
        "zInter",
        "zPop",
        "zRange",
        "zRangeByScore",
        "zRevRangeByScore",
        "zRangeByLex",
        "zRank",
        "zRevRank",
        "zRem",
        "zDelete",
        "zRemove",
        "zRemRangeByRank",
        "zDeleteRangeByRank",
        "zRemRangeByScore",
        "zDeleteRangeByScore",
        "zRemoveRangeByScore",
        "zRevRange",
        "zScore",
        "zunionstore",
        "zUnion",
        "zScan",
    ]
    .into_iter()
    .map(str::to_ascii_lowercase)
    .collect()
});

#[derive(Default, Clone)]
pub struct RedisPlugin;

impl Plugin for RedisPlugin {
    #[inline]
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["Redis"])
    }

    #[inline]
    fn function_name_prefix(&self) -> Option<&'static str> {
        None
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)> {
        match (class_name, function_name) {
            (Some("Redis"), "__construct") => Some(self.hook_redis_construct()),
            (Some(class_name @ "Redis"), f)
                if ["connect", "open", "pconnect", "popen"].contains(&f) =>
            {
                Some(self.hook_redis_connect(class_name, function_name))
            }
            (Some(class_name @ "Redis"), f) if REDIS_COMMANDS.contains(&f.to_ascii_lowercase()) => {
                Some(self.hook_redis_methods(class_name, function_name))
            }
            _ => None,
        }
    }
}

impl RedisPlugin {
    /// TODO Support first optional argument as config for phpredis 6.0+.
    /// <https://github.com/phpredis/phpredis/blob/cc2383f07666e6afefd7b58995fb607d9967d650/README.markdown#example-1>
    fn hook_redis_construct(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                let this = get_this_mut(execute_data)?;
                hack_free(this, Some(redis_dtor));

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn hook_redis_connect(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                if execute_data.num_args() < 2 {
                    debug!("argument count less than 2, skipped.");
                    return Ok(Box::new(()));
                }

                let host = {
                    let mut f = || {
                        Ok::<_, anyhow::Error>(
                            execute_data
                                .get_parameter(0)
                                .as_z_str()
                                .context("isn't string")?
                                .to_str()?
                                .to_owned(),
                        )
                    };
                    match f() {
                        Ok(host) => host,
                        Err(err) => {
                            warn!(?err, "parse first argument to host failed, skipped.");
                            return Ok(Box::new(()));
                        }
                    }
                };
                let port = {
                    let mut f = || {
                        execute_data
                            .get_parameter(1)
                            .as_long()
                            .context("isn't long")
                    };
                    match f() {
                        Ok(port) => port,
                        Err(err) => {
                            warn!(?err, "parse second argument to port failed, skipped.");
                            return Ok(Box::new(()));
                        }
                    }
                };

                let this = get_this_mut(execute_data)?;
                let addr = format!("{}:{}", host, port);
                debug!(addr, "Get redis peer");
                PEER_MAP.insert(this.handle(), Peer { addr: addr.clone() });

                let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    Ok(ctx.create_exit_span(&format!("{}->{}", class_name, function_name), &addr))
                })?;

                span.with_span_object_mut(|span| {
                    span.set_span_layer(SpanLayer::Cache);
                    span.component_id = COMPONENT_PHP_REDIS_ID;
                    span.add_tag("db.type", "redis");
                });

                Ok(Box::new(span))
            }),
            Box::new(after_hook),
        )
    }

    fn hook_redis_methods(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let handle = get_this_mut(execute_data)?.handle();
                debug!(handle, function_name, "call redis method");
                let peer = PEER_MAP
                    .get(&handle)
                    .map(|r| r.value().addr.clone())
                    .unwrap_or_default();

                let command = generate_command(&function_name, execute_data)?;

                debug!(handle, function_name, command, "call redis command");

                let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    Ok(ctx.create_exit_span(&format!("{}->{}", class_name, function_name), &peer))
                })?;

                span.with_span_object_mut(|span| {
                    span.set_span_layer(SpanLayer::Cache);
                    span.component_id = COMPONENT_PHP_REDIS_ID;
                    span.add_tag("db.type", "redis");
                    span.add_tag("redis.command", command);
                });

                Ok(Box::new(span))
            }),
            Box::new(after_hook),
        )
    }
}

struct Peer {
    addr: String,
}

fn hack_free(this: &mut ZObj, new_free: sys::zend_object_free_obj_t) {
    let handle = this.handle();

    unsafe {
        let ori_free = (*(*this.as_mut_ptr()).handlers).free_obj;
        FREE_MAP.insert(handle, ori_free);
        (*((*this.as_mut_ptr()).handlers as *mut sys::zend_object_handlers)).free_obj = new_free;
    }
}

unsafe extern "C" fn redis_dtor(object: *mut sys::zend_object) {
    debug!("call Redis free");

    let handle = ZObj::from_ptr(object).handle();

    PEER_MAP.remove(&handle);
    if let Some((_, Some(free))) = FREE_MAP.remove(&handle) {
        free(object);
    }
}

fn generate_command(function_name: &str, execute_data: &mut ExecuteData) -> anyhow::Result<String> {
    let num_args = execute_data.num_args();
    let mut args = Vec::with_capacity(num_args + 1);
    args.push(ZVal::from(function_name));

    for i in 0..num_args {
        let arg = execute_data.get_parameter(i).clone();
        args.push(arg);
    }

    Ok(json_encode_values(&args)?)
}

fn after_hook(
    _request_id: Option<i64>, span: Box<dyn Any>, _execute_data: &mut ExecuteData,
    _return_value: &mut ZVal,
) -> anyhow::Result<()> {
    let mut span = span.downcast::<Span>().unwrap();

    let ex = unsafe { ZObj::try_from_mut_ptr(eg!(exception)) };
    if let Some(ex) = ex {
        span.with_span_object_mut(|span| {
            span.is_error = true;

            let mut logs = Vec::new();
            if let Ok(class_name) = ex.get_class().get_name().to_str() {
                logs.push(("Exception Class", class_name.to_owned()));
            }
            if let Some(message) = ex.get_property("message").as_z_str() {
                if let Ok(message) = message.to_str() {
                    logs.push(("Exception Message", message.to_owned()));
                }
            }
            if !logs.is_empty() {
                span.add_log(logs);
            }
        });
    }

    Ok(())
}

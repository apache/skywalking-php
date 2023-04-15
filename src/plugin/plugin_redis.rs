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
    component::COMPONENT_PHP_REDIS_ID,
    context::RequestContext,
    execute::{get_this_mut, AfterExecuteHook, BeforeExecuteHook, Noop},
    tag::{TAG_CACHE_CMD, TAG_CACHE_KEY, TAG_CACHE_OP, TAG_CACHE_TYPE},
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
use std::{any::Any, collections::HashMap};
use tracing::{debug, warn};

static PEER_MAP: Lazy<DashMap<u32, Peer>> = Lazy::new(Default::default);

static FREE_MAP: Lazy<DashMap<u32, sys::zend_object_free_obj_t>> = Lazy::new(Default::default);

static REDIS_READ_MAPPING: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    [
        ("blpop", "BLPOP"),
        ("brpop", "BRPOP"),
        ("get", "GET"),
        ("getbit", "GETBIT"),
        ("getkeys", "KEYS"),
        ("getmultiple", "MGET"),
        ("getrange", "GETRANGE"),
        ("hexists", "HEXISTS"),
        ("hget", "HGET"),
        ("hgetall", "HGETALL"),
        ("hkeys", "HKEYS"),
        ("hlen", "HLEN"),
        ("hmget", "HMGET"),
        ("hscan", "HSCAN"),
        ("hstrlen", "HSTRLEN"),
        ("hvals", "HVALS"),
        ("keys", "KEYS"),
        ("lget", "LGET"),
        ("lgetrange", "LGETRANGE"),
        ("llen", "LLEN"),
        ("lrange", "LRANGE"),
        ("lsize", "LSIZE"),
        ("mget", "MGET"),
        ("mget", "MGET"),
        ("scontains", "SCONTAINS"),
        ("sgetmembers", "SGETMEMBERS"),
        ("sismember", "SISMEMBER"),
        ("smembers", "SMEMBERS"),
        ("sscan", "SSCAN"),
        ("ssize", "SSIZE"),
        ("strlen", "STRLEN"),
        ("substr", "GETRANGE"),
        ("zcount", "ZCOUNT"),
        ("zrange", "ZRANGE"),
        ("zrangebylex", "ZRANGEBYLEX"),
        ("zrangebyscore", "ZRANGEBYSCORE"),
        ("zscan", "ZSCAN"),
        ("zsize", "ZSIZE"),
    ]
    .into_iter()
    .collect()
});

static REDIS_WRITE_MAPPING: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    [
        ("append", "APPEND"),
        ("brpoplpush", "BRPOPLPUSH"),
        ("decr", "DECR"),
        ("decrby", "DECRBY"),
        ("del", "DEL"),
        ("delete", "DEL"),
        ("hdel", "HDEL"),
        ("hincrby", "HINCRBY"),
        ("hincrbyfloat", "HINCRBYFLOAT"),
        ("hmset", "HMSET"),
        ("hset", "HSET"),
        ("hsetnx", "HSETNX"),
        ("incr", "INCR"),
        ("incrby", "INCRBY"),
        ("incrbyfloat", "INCRBYFLOAT"),
        ("linsert", "LINSERT"),
        ("lpush", "LPUSH"),
        ("lpushx", "LPUSHX"),
        ("lrem", "LREM"),
        ("lremove", "LREMOVE"),
        ("lset", "LSET"),
        ("ltrim", "LTRIM"),
        ("listtrim", "LISTTRIM"),
        ("mset", "MSET"),
        ("msetnx", "MSETNX"),
        ("psetex", "PSETEX"),
        ("rpoplpush", "RPOPLPUSH"),
        ("rpush", "RPUSH"),
        ("rpushx", "RPUSHX"),
        ("randomkey", "RANDOMKEY"),
        ("sadd", "SADD"),
        ("sinter", "SINTER"),
        ("sinterstore", "SINTERSTORE"),
        ("smove", "SMOVE"),
        ("srandmember", "SRANDMEMBER"),
        ("srem", "SREM"),
        ("sremove", "SREMOVE"),
        ("set", "SET"),
        ("setbit", "SETBIT"),
        ("setex", "SETEX"),
        ("setnx", "SETNX"),
        ("setrange", "SETRANGE"),
        ("settimeout", "SETTIMEOUT"),
        ("sort", "SORT"),
        ("unlink", "UNLINK"),
        ("zadd", "ZADD"),
        ("zdelete", "ZDELETE"),
        ("zdeleterangebyrank", "ZDELETERANGEBYRANK"),
        ("zdeleterangebyscore", "ZDELETERANGEBYSCORE"),
        ("zincrby", "ZINCRBY"),
        ("zrem", "ZREM"),
        ("zremrangebyrank", "ZREMRANGEBYRANK"),
        ("zremrangebyscore", "ZREMRANGEBYSCORE"),
        ("zremove", "ZREMOVE"),
        ("zremoverangebyscore", "ZREMOVERANGEBYSCORE"),
    ]
    .into_iter()
    .collect()
});

static REDIS_OTHER_MAPPING: Lazy<HashMap<&str, &str>> =
    Lazy::new(|| [("auth", "AUTH")].into_iter().collect());

static REDIS_ALL_MAPPING: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    let mut commands = HashMap::with_capacity(REDIS_READ_MAPPING.len() + REDIS_WRITE_MAPPING.len());
    commands.extend(REDIS_READ_MAPPING.iter());
    commands.extend(REDIS_WRITE_MAPPING.iter());
    commands.extend(REDIS_OTHER_MAPPING.iter());
    commands
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
            (Some(class_name @ "Redis"), f)
                if REDIS_ALL_MAPPING.contains_key(&*f.to_ascii_lowercase()) =>
            {
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
                    let f = || {
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
                    let f = || {
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

                let mut span_object = span.span_object_mut();
                span_object.set_span_layer(SpanLayer::Cache);
                span_object.component_id = COMPONENT_PHP_REDIS_ID;
                span_object.add_tag(TAG_CACHE_TYPE, "redis");
                drop(span_object);

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

                let function_name_key = &*function_name.to_ascii_lowercase();

                let op = if REDIS_READ_MAPPING.contains_key(function_name_key) {
                    Some("read")
                } else if REDIS_WRITE_MAPPING.contains_key(function_name_key) {
                    Some("write")
                } else {
                    None
                };

                let key = op
                    .and_then(|_| execute_data.get_parameter(0).as_z_str())
                    .and_then(|s| s.to_str().ok());

                debug!(handle, cmd = function_name, key, op, "call redis command");

                let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    Ok(ctx.create_exit_span(&format!("{}->{}", class_name, function_name), &peer))
                })?;

                let mut span_object = span.span_object_mut();
                span_object.set_span_layer(SpanLayer::Cache);
                span_object.component_id = COMPONENT_PHP_REDIS_ID;
                span_object.add_tag(TAG_CACHE_TYPE, "redis");
                span_object.add_tag(
                    TAG_CACHE_CMD,
                    *REDIS_ALL_MAPPING.get(function_name_key).unwrap(),
                );
                if let Some(op) = op {
                    span_object.add_tag(TAG_CACHE_OP, op);
                }
                if let Some(key) = key {
                    span_object.add_tag(TAG_CACHE_KEY, key)
                }
                drop(span_object);

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

fn after_hook(
    _request_id: Option<i64>, span: Box<dyn Any>, _execute_data: &mut ExecuteData,
    _return_value: &mut ZVal,
) -> crate::Result<()> {
    let mut span = span.downcast::<Span>().unwrap();

    let ex = unsafe { ZObj::try_from_mut_ptr(eg!(exception)) };
    if let Some(ex) = ex {
        let mut span_object = span.span_object_mut();
        span_object.is_error = true;

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
            span_object.add_log(logs);
        }
    }

    Ok(())
}

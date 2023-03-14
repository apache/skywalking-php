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

// TODO Need to be improved.

use super::Plugin;
use crate::{
    component::COMPONENT_PHP_PREDIS_ID,
    context::RequestContext,
    execute::{get_this_mut, validate_num_args, AfterExecuteHook, BeforeExecuteHook},
    tag::{TAG_CACHE_CMD, TAG_CACHE_KEY, TAG_CACHE_OP, TAG_CACHE_TYPE},
};
use anyhow::Context;
use once_cell::sync::Lazy;
use skywalking::{skywalking_proto::v3::SpanLayer, trace::span::Span};
use std::collections::HashSet;
use tracing::debug;

pub static REDIS_READ_COMMANDS: Lazy<HashSet<&str>> = Lazy::new(|| {
    [
        "BLPOP",
        "BRPOP",
        "GET",
        "GETBIT",
        "GETRANGE",
        "HEXISTS",
        "HGET",
        "HGETALL",
        "HKEYS",
        "HLEN",
        "HMGET",
        "HSCAN",
        "HSTRLEN",
        "HVALS",
        "KEYS",
        "LGET",
        "LGETRANGE",
        "LLEN",
        "LRANGE",
        "LSIZE",
        "MGET",
        "SCONTAINS",
        "SGETMEMBERS",
        "SISMEMBER",
        "SMEMBERS",
        "SSCAN",
        "SSIZE",
        "STRLEN",
        "ZCOUNT",
        "ZRANGE",
        "ZRANGEBYLEX",
        "ZRANGEBYSCORE",
        "ZSCAN",
        "ZSIZE",
    ]
    .into_iter()
    .collect()
});

pub static REDIS_WRITE_COMMANDS: Lazy<HashSet<&str>> = Lazy::new(|| {
    [
        "APPEND",
        "BRPOPLPUSH",
        "DECR",
        "DECRBY",
        "DEL",
        "DELETE",
        "HDEL",
        "HINCRBY",
        "HINCRBYFLOAT",
        "HMSET",
        "HSET",
        "HSETNX",
        "INCR",
        "INCRBY",
        "INCRBYFLOAT",
        "LINSERT",
        "LPUSH",
        "LPUSHX",
        "LREM",
        "LREMOVE",
        "LSET",
        "LTRIM",
        "LISTTRIM",
        "MSET",
        "MSETNX",
        "PSETEX",
        "RPOPLPUSH",
        "RPUSH",
        "RPUSHX",
        "RANDOMKEY",
        "SADD",
        "SINTER",
        "SINTERSTORE",
        "SMOVE",
        "SRANDMEMBER",
        "SREM",
        "SREMOVE",
        "SET",
        "SETBIT",
        "SETEX",
        "SETNX",
        "SETRANGE",
        "SETTIMEOUT",
        "SORT",
        "UNLINK",
        "ZADD",
        "ZDELETE",
        "ZDELETERANGEBYRANK",
        "ZDELETERANGEBYSCORE",
        "ZINCRBY",
        "ZREM",
        "ZREMRANGEBYRANK",
        "ZREMRANGEBYSCORE",
        "ZREMOVE",
        "ZREMOVERANGEBYSCORE",
    ]
    .into_iter()
    .collect()
});

#[derive(Default, Clone)]
pub struct PredisPlugin;

impl Plugin for PredisPlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["Predis\\Connection\\AbstractConnection"])
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
            (
                Some(class_name @ "Predis\\Connection\\AbstractConnection"),
                function_name @ "executeCommand",
            ) => Some(self.hook_predis_execute_command(class_name, function_name)),
            _ => None,
        }
    }
}

impl PredisPlugin {
    fn hook_predis_execute_command(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                validate_num_args(execute_data, 1)?;

                let this = get_this_mut(execute_data)?;
                let parameters = this.get_mut_property("parameters").expect_mut_z_obj()?;
                let parameters = parameters
                    .get_mut_property("parameters")
                    .expect_mut_z_arr()?;
                let host = parameters
                    .get_mut("host")
                    .context("host not found")?
                    .expect_z_str()?
                    .to_str()?;
                let port = parameters
                    .get_mut("port")
                    .context("port not found")?
                    .expect_long()?;
                let peer = format!("{}:{}", host, port);

                let handle = this.handle();
                let command = execute_data.get_parameter(0).expect_mut_z_obj()?;
                let command_class_name = command
                    .get_class()
                    .get_name()
                    .to_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_default();

                let id = command.call("getid", [])?;
                let cmd = id.expect_z_str()?.to_str()?.to_ascii_uppercase();

                let mut arguments = command.call("getarguments", [])?;
                let arguments = arguments.expect_mut_z_arr()?;

                let op = if REDIS_READ_COMMANDS.contains(&*cmd) {
                    Some("read")
                } else if REDIS_WRITE_COMMANDS.contains(&*cmd) {
                    Some("write")
                } else {
                    None
                };

                let key = op
                    .and_then(|_| arguments.get(0))
                    .and_then(|arg| arg.as_z_str())
                    .and_then(|s| s.to_str().ok());

                debug!(handle, cmd, key, op, "call redis command");

                let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    Ok(ctx.create_exit_span(
                        &format!("{}->{}({})", class_name, function_name, command_class_name),
                        &peer,
                    ))
                })?;

                let mut span_object = span.span_object_mut();
                span_object.set_span_layer(SpanLayer::Cache);
                span_object.component_id = COMPONENT_PHP_PREDIS_ID;
                span_object.add_tag(TAG_CACHE_TYPE, "redis");
                span_object.add_tag(TAG_CACHE_CMD, cmd);
                if let Some(op) = op {
                    span_object.add_tag(TAG_CACHE_OP, op);
                };
                if let Some(key) = key {
                    span_object.add_tag(TAG_CACHE_KEY, key)
                }
                drop(span_object);

                Ok(Box::new(span))
            }),
            Box::new(move |_, span, _, return_value| {
                let mut span = span.downcast::<Span>().unwrap();

                let typ = return_value.get_type_info();
                if typ.is_null() || typ.is_false() {
                    span.span_object_mut().is_error = true;
                }

                Ok(())
            }),
        )
    }
}

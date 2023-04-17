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
    component::COMPONENT_PHP_PREDIS_ID,
    context::RequestContext,
    execute::{get_this_mut, validate_num_args, AfterExecuteHook, BeforeExecuteHook},
    tag::{TAG_CACHE_CMD, TAG_CACHE_KEY, TAG_CACHE_OP, TAG_CACHE_TYPE},
};
use once_cell::sync::Lazy;
use phper::{eg, functions::call, values::ZVal};
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

static REDIS_OTHER_COMMANDS: Lazy<HashSet<&str>> = Lazy::new(|| ["AUTH"].into_iter().collect());

static REDIS_ALL_COMMANDS: Lazy<HashSet<&str>> = Lazy::new(|| {
    let mut commands = HashSet::with_capacity(
        REDIS_READ_COMMANDS.len() + REDIS_WRITE_COMMANDS.len() + REDIS_OTHER_COMMANDS.len(),
    );
    commands.extend(REDIS_READ_COMMANDS.iter());
    commands.extend(REDIS_WRITE_COMMANDS.iter());
    commands.extend(REDIS_OTHER_COMMANDS.iter());
    commands
});

#[derive(Default, Clone)]
pub struct PredisPlugin;

impl Plugin for PredisPlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["Predis\\Client"])
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
            (Some(class_name @ "Predis\\Client"), function_name)
                if REDIS_ALL_COMMANDS.contains(&*function_name.to_ascii_uppercase()) =>
            {
                Some(self.hook_predis_execute_command(class_name, function_name))
            }
            _ => None,
        }
    }
}

enum ConnectionType {
    AbstractConnection,
    Unknown,
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
                let handle = this.handle();
                let connection = this.call("getConnection", [])?;

                let peer = Self::get_peer(connection)?;

                let cmd = function_name.to_ascii_uppercase();

                let op = if REDIS_READ_COMMANDS.contains(&*cmd) {
                    Some("read")
                } else if REDIS_WRITE_COMMANDS.contains(&*cmd) {
                    Some("write")
                } else {
                    None
                };

                let key = op
                    .and_then(|_| execute_data.get_parameter(0).as_z_str())
                    .and_then(|s| s.to_str().ok());

                debug!(handle, cmd, key, op, "call redis command");

                let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    Ok(ctx.create_exit_span(&format!("{}->{}", class_name, function_name), &peer))
                })?;

                let mut span_object = span.span_object_mut();
                span_object.set_span_layer(SpanLayer::Cache);
                span_object.component_id = COMPONENT_PHP_PREDIS_ID;
                span_object.add_tag(TAG_CACHE_TYPE, "redis");
                span_object.add_tag(TAG_CACHE_CMD, cmd);
                if let Some(op) = op {
                    span_object.add_tag(TAG_CACHE_OP, op);
                }
                if let Some(key) = key {
                    span_object.add_tag(TAG_CACHE_KEY, key)
                }
                drop(span_object);

                Ok(Box::new(span))
            }),
            Box::new(move |_, span, _, return_value| {
                let mut span = span.downcast::<Span>().unwrap();

                let exception = unsafe { eg!(exception) };

                debug!(?return_value, ?exception, "predis after execute command");

                let typ = return_value.get_type_info();
                if !exception.is_null() || typ.is_false() {
                    span.span_object_mut().is_error = true;
                }

                Ok(())
            }),
        )
    }

    fn get_peer(mut connection: ZVal) -> crate::Result<String> {
        let connection_type = Self::infer_connection_type(connection.clone())?;
        match connection_type {
            ConnectionType::AbstractConnection => {
                let connection = connection.expect_mut_z_obj()?;

                let mut parameters = connection.call("getParameters", [])?;
                let parameters = parameters.expect_mut_z_obj()?;

                let host = parameters.call("__get", [ZVal::from("host")])?;
                let host = host.expect_z_str()?.to_str()?;

                let port = parameters.call("__get", [ZVal::from("port")])?;
                let port = port.expect_long()?;

                Ok(format!("{}:{}", host, port))
            }
            ConnectionType::Unknown => Ok("unknown:0".to_owned()),
        }
    }

    fn infer_connection_type(connection: ZVal) -> crate::Result<ConnectionType> {
        let is_abstract_connection = call(
            "is_a",
            [
                connection,
                ZVal::from("Predis\\Connection\\AbstractConnection"),
            ],
        )?;
        if is_abstract_connection.as_bool() == Some(true) {
            return Ok(ConnectionType::AbstractConnection);
        }
        Ok(ConnectionType::Unknown)
    }
}

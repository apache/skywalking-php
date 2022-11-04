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
use crate::execute::{get_this_mut, AfterExecuteHook, BeforeExecuteHook, Noop};
use anyhow::Context;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use phper::{objects::ZObj, sys, values::ExecuteData};
use tracing::{debug, warn};

static PEER_MAP: Lazy<DashMap<u32, Peer>> = Lazy::new(Default::default);
static FREE_MAP: Lazy<DashMap<u32, sys::zend_object_free_obj_t>> = Lazy::new(Default::default);

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
        debug!(function_name, "REDIS COMMAND");
        match (class_name, function_name) {
            (Some("Redis"), "__construct") => Some(self.hook_redis_construct()),
            (Some("Redis"), f) if ["connect", "open", "pconnect", "popen"].contains(&f) => {
                Some(self.hook_redis_connect())
            }
            (Some("Redis"), f) if ["get", "mget"].contains(&f) => {
                Some(self.hook_redis_methods(function_name))
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

    fn hook_redis_connect(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
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
                        Ok::<_, anyhow::Error>(
                            execute_data
                                .get_parameter(1)
                                .as_long()
                                .context("isn't long")?,
                        )
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
                PEER_MAP.insert(this.handle(), Peer { addr });

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn hook_redis_methods(
        &self, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let handle = get_this_mut(execute_data)?.handle();

                debug!(handle, function_name, "call redis method");

                let command = generate_command(&function_name, execute_data)?;

                debug!(handle, function_name, command, "call redis command");

                Ok(Box::new(()))
            }),
            Noop::noop(),
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
    args.push(function_name.to_owned());

    for i in 0..num_args {
        let mut arg = execute_data.get_parameter(i).clone();
        arg.convert_to_string();
        args.push(arg.as_z_str().unwrap().to_str()?.to_string());
    }

    Ok(args.join(" "))
}

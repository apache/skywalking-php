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
    execute::{get_this_mut, validate_num_args, AfterExecuteHook, BeforeExecuteHook, Noop},
    plugin::Plugin,
    request::{
        HACK_SWOOLE_ON_REQUEST_FUNCTION_NAME, IS_SWOOLE, ORI_SWOOLE_ON_REQUEST,
        SWOOLE_RESPONSE_STATUS_MAP,
    },
};
use phper::{strings::ZString, values::ZVal};
use std::{mem::replace, sync::atomic::Ordering};

#[derive(Default, Clone)]
pub struct SwooleServerPlugin;

impl Plugin for SwooleServerPlugin {
    #[inline]
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["Swoole\\Server"])
    }

    #[inline]
    fn function_name_prefix(&self) -> Option<&'static str> {
        None
    }

    fn hook(
        &self, _class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)> {
        match function_name {
            "on" => Some(self.hook_on()),
            _ => None,
        }
    }
}

impl SwooleServerPlugin {
    fn hook_on(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 2)?;

                let on = execute_data.get_parameter(0);
                if !on.as_z_str().map(|s| s == b"request").unwrap_or_default() {
                    return Ok(Box::new(()));
                }

                // Hack the closure with the
                // [`crate::request::skywalking_hack_swoole_on_request`].
                let closure = execute_data.get_mut_parameter(1);
                let ori_closure = replace(
                    closure,
                    ZVal::from(ZString::new(HACK_SWOOLE_ON_REQUEST_FUNCTION_NAME)),
                );

                ORI_SWOOLE_ON_REQUEST.store(
                    Box::into_raw(Box::new(ori_closure)).cast(),
                    Ordering::Relaxed,
                );
                IS_SWOOLE.store(true, Ordering::Relaxed);

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }
}

#[derive(Default, Clone)]
pub struct SwooleHttpResponsePlugin;

impl Plugin for SwooleHttpResponsePlugin {
    #[inline]
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["Swoole\\Http\\Response"])
    }

    #[inline]
    fn function_name_prefix(&self) -> Option<&'static str> {
        None
    }

    fn hook(
        &self, _class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)> {
        match function_name {
            "status" => Some(self.hook_status()),
            _ => None,
        }
    }
}

impl SwooleHttpResponsePlugin {
    fn hook_status(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 1)?;

                let fd = get_this_mut(execute_data)?
                    .get_mut_property("fd")
                    .expect_long()?;

                let status = execute_data.get_parameter(0);
                let status = status
                    .as_long()
                    .map(|status| status as i32)
                    .or_else(|| {
                        status
                            .as_z_str()
                            .and_then(|status| status.to_str().ok())
                            .and_then(|status| status.parse::<i32>().ok())
                    })
                    .unwrap_or_default();

                SWOOLE_RESPONSE_STATUS_MAP.insert(fd, status);

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }
}

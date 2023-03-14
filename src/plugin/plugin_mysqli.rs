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
    component::COMPONENT_PHP_MYSQLI_ID,
    context::RequestContext,
    execute::{get_this_mut, AfterExecuteHook, BeforeExecuteHook, Noop},
};
use anyhow::Context;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use phper::{objects::ZObj, sys};
use skywalking::{skywalking_proto::v3::SpanLayer, trace::span::Span};
use tracing::debug;

static MYSQL_MAP: Lazy<DashMap<u32, MySQLInfo>> = Lazy::new(Default::default);

static DTOR_MAP: Lazy<DashMap<u32, sys::zend_object_dtor_obj_t>> = Lazy::new(Default::default);

#[derive(Default, Clone)]
pub struct MySQLImprovedPlugin;

impl Plugin for MySQLImprovedPlugin {
    #[inline]
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["mysqli"])
    }

    #[inline]
    fn function_name_prefix(&self) -> Option<&'static str> {
        None
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)> {
        match (class_name, function_name) {
            (Some("mysqli"), "__construct") => Some(self.hook_mysqli_construct()),
            (Some("mysqli"), f) if ["query"].contains(&f) => {
                Some(self.hook_mysqli_methods(function_name))
            }
            _ => None,
        }
    }
}

impl MySQLImprovedPlugin {
    fn hook_mysqli_construct(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                let this = get_this_mut(execute_data)?;
                let handle = this.handle();
                hack_dtor(this, Some(mysqli_dtor));

                let mut info: MySQLInfo = MySQLInfo {
                    hostname: "127.0.0.1".to_string(),
                    port: 3306,
                };

                let num_args = execute_data.num_args();
                if num_args >= 1 {
                    // host only
                    let hostname = execute_data.get_parameter(0);
                    let hostname = hostname
                        .as_z_str()
                        .context("hostname isn't str")?
                        .to_str()?;
                    debug!(hostname, "mysqli hostname");

                    info.hostname = hostname.to_owned();
                }
                if num_args >= 5 {
                    let port = execute_data.get_parameter(4);
                    let port = port.as_long().context("port isn't str")?;
                    debug!(port, "mysqli port");
                    info.port = port
                }

                MYSQL_MAP.insert(handle, info);
                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn hook_mysqli_methods(
        &self, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let this = get_this_mut(execute_data)?;
                let handle = this.handle();

                debug!(handle, function_name, "call mysqli method");

                let mut span = with_info(handle, |info| {
                    create_mysqli_exit_span(request_id, "mysqli", &function_name, info)
                })?;

                if execute_data.num_args() >= 1 {
                    if let Some(statement) = execute_data.get_parameter(0).as_z_str() {
                        span.add_tag("db.statement", statement.to_str()?);
                    }
                }

                Ok(Box::new(span) as _)
            }),
            Noop::noop(),
        )
    }
}

fn create_mysqli_exit_span(
    request_id: Option<i64>, class_name: &str, function_name: &str, info: &MySQLInfo,
) -> anyhow::Result<Span> {
    RequestContext::try_with_global_ctx(request_id, |ctx| {
        let mut span = ctx.create_exit_span(
            &format!("{}->{}", class_name, function_name),
            &format!("{}:{}", info.hostname, info.port),
        );

        let mut span_object = span.span_object_mut();
        span_object.set_span_layer(SpanLayer::Database);
        span_object.component_id = COMPONENT_PHP_MYSQLI_ID;
        span_object.add_tag("db.type", "mysql");
        drop(span_object);

        Ok(span)
    })
}

fn with_info<T>(handle: u32, f: impl FnOnce(&MySQLInfo) -> anyhow::Result<T>) -> anyhow::Result<T> {
    MYSQL_MAP
        .get(&handle)
        .map(|r| f(r.value()))
        .context("info not exists")?
}

struct MySQLInfo {
    hostname: String,
    port: i64,
}

fn hack_dtor(this: &mut ZObj, new_dtor: sys::zend_object_dtor_obj_t) {
    let handle = this.handle();

    unsafe {
        let ori_dtor = (*(*this.as_mut_ptr()).handlers).dtor_obj;
        DTOR_MAP.insert(handle, ori_dtor);
        (*((*this.as_mut_ptr()).handlers as *mut sys::zend_object_handlers)).dtor_obj = new_dtor;
    }
}

unsafe extern "C" fn mysqli_dtor(object: *mut sys::zend_object) {
    debug!("call mysqli dtor");
    dtor(object);
}

unsafe extern "C" fn dtor(object: *mut sys::zend_object) {
    let handle = ZObj::from_ptr(object).handle();

    MYSQL_MAP.remove(&handle);
    if let Some((_, Some(dtor))) = DTOR_MAP.remove(&handle) {
        dtor(object);
    }
}

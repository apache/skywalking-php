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
    component::COMPONENT_PHP_MYSQLI_ID,
    context::RequestContext,
    execute::{AfterExecuteHook, BeforeExecuteHook},
};
use phper::{
    alloc::ToRefOwned,
    functions::call,
    objects::ZObj,
    values::{ExecuteData, ZVal},
};
use skywalking::{
    proto::v3::SpanLayer,
    trace::span::{HandleSpanObject, Span},
};
use tracing::{debug, error};

#[derive(Default, Clone)]
pub struct MySQLImprovedPlugin;

impl Plugin for MySQLImprovedPlugin {
    #[inline]
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["mysqli"])
    }

    #[inline]
    fn function_name_prefix(&self) -> Option<&'static str> {
        Some("mysqli_")
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)> {
        match (class_name, function_name) {
            (Some("mysqli"), "__construct" | "real_connect") => {
                Some(self.hook_mysqli_connect(class_name, function_name, ApiStyle::OO))
            }
            (None, "mysqli_connect" | "mysqli_real_connect") => {
                Some(self.hook_mysqli_connect(class_name, function_name, ApiStyle::Procedural))
            }
            (Some("mysqli"), f)
                if [
                    "query",
                    "execute_query",
                    "multi_query",
                    "real_query",
                    "prepare",
                ]
                .contains(&f) =>
            {
                Some(self.hook_mysqli_methods(class_name, function_name, ApiStyle::OO))
            }
            (None, f)
                if [
                    "mysqli_query",
                    "mysqli_execute_query",
                    "mysqli_multi_query",
                    "mysqli_real_query",
                    "mysqli_prepare",
                ]
                .contains(&f) =>
            {
                Some(self.hook_mysqli_methods(class_name, function_name, ApiStyle::Procedural))
            }
            _ => None,
        }
    }
}

impl MySQLImprovedPlugin {
    fn hook_mysqli_connect(
        &self, class_name: Option<&str>, function_name: &str, style: ApiStyle,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.map(ToOwned::to_owned);
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                // Sometimes the connection is failed. Therefore, first assemble the peer from
                // the parameters to prevent assembly failure in the after hook.
                let peer = get_peer_by_parameters(execute_data, style);

                let span = create_mysqli_exit_span(
                    request_id,
                    class_name.as_deref(),
                    &function_name,
                    &peer,
                    style,
                )?;

                Ok(Box::new(span))
            }),
            Box::new(move |_, span, execute_data, return_value| {
                let mut span = span.downcast::<Span>().unwrap();

                // Reset the peer here, it should be more precise.
                if let Some(b) = return_value.as_bool() {
                    if !b {
                        span.span_object_mut().is_error = true;
                    }
                }
                if let Some(this) = return_value.as_mut_z_obj() {
                    if let Some(peer) = get_peer_by_this(this) {
                        span.span_object_mut().peer = peer;
                    }
                } else {
                    match style.get_this_mut(execute_data) {
                        Ok(this) => {
                            if let Some(peer) = get_peer_by_this(this) {
                                span.span_object_mut().peer = peer;
                            }
                        }
                        Err(err) => {
                            error!(?err, "reset peer failed");
                        }
                    }
                }

                log_exception(&mut *span);
                Ok(())
            }),
        )
    }

    fn hook_mysqli_methods(
        &self, class_name: Option<&str>, function_name: &str, style: ApiStyle,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.map(ToOwned::to_owned);
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let this = style.get_this_mut(execute_data)?;
                let handle = this.handle();

                debug!(handle, class_name, function_name, "call mysqli method");

                let peer = &get_peer_by_this(this).unwrap_or_default();
                let mut span = create_mysqli_exit_span(
                    request_id,
                    class_name.as_deref(),
                    &function_name,
                    peer,
                    style,
                )?;

                if execute_data.num_args() >= 1 {
                    if let Some(statement) = execute_data.get_parameter(0).as_z_str() {
                        span.add_tag("db.statement", statement.to_str()?);
                    }
                }

                Ok(Box::new(span) as _)
            }),
            Box::new(move |_, span, _, return_value| {
                let mut span = span.downcast::<Span>().unwrap();
                if let Some(b) = return_value.as_bool() {
                    if !b {
                        span.span_object_mut().is_error = true;
                    }
                }
                log_exception(&mut *span);
                Ok(())
            }),
        )
    }
}

fn create_mysqli_exit_span(
    request_id: Option<i64>, class_name: Option<&str>, function_name: &str, peer: &str,
    style: ApiStyle,
) -> anyhow::Result<Span> {
    RequestContext::try_with_global_ctx(request_id, |ctx| {
        let mut span = ctx.create_exit_span(
            &style.generate_operation_name(class_name, function_name),
            peer,
        );

        let span_object = span.span_object_mut();
        span_object.set_span_layer(SpanLayer::Database);
        span_object.component_id = COMPONENT_PHP_MYSQLI_ID;
        span_object.add_tag("db.type", "mysql");

        Ok(span)
    })
}

fn get_peer_by_this(this: &mut ZObj) -> Option<String> {
    let handle = this.handle();

    debug!(handle, "start to call mysqli_get_host_info");

    let host_info = match call("mysqli_get_host_info", [ZVal::from(this.to_ref_owned())]) {
        Ok(host_info) => host_info,
        Err(err) => {
            error!(handle, ?err, "call mysqli_get_host_info failed");
            return None;
        }
    };

    host_info
        .as_z_str()
        .and_then(|info| info.to_str().ok())
        .and_then(|info| info.split(' ').next())
        .map(ToOwned::to_owned)
        .map(|mut info| {
            if !info.contains(':') {
                info.push_str(":3306");
            }
            info
        })
}

fn get_peer_by_parameters(execute_data: &mut ExecuteData, style: ApiStyle) -> String {
    let mut peer = "".to_owned();

    if style.validate_num_args(execute_data, 1).is_ok() {
        peer.push_str(
            style
                .get_mut_parameter(execute_data, 0)
                .as_z_str()
                .and_then(|s| s.to_str().ok())
                .unwrap_or_default(),
        );
    }

    if !peer.is_empty() {
        let port = style.get_mut_parameter(execute_data, 4);

        #[allow(clippy::manual_map)]
        let port = if let Some(port) = port.as_z_str() {
            port.to_str().ok().map(ToOwned::to_owned)
        } else if let Some(port) = port.as_long() {
            Some(port.to_string())
        } else {
            None
        };

        peer.push(':');
        peer.push_str(port.as_deref().unwrap_or("3306"));
    }

    peer
}

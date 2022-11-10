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
};
use anyhow::Context;
use phper::arrays::ZArr;
use skywalking::{skywalking_proto::v3::SpanLayer, trace::span::Span};

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
            (Some(class_name @ "Predis\\Connection\\AbstractConnection"), "executeCommand") => {
                Some(self.hook_predis_execute_command(class_name))
            }
            _ => None,
        }
    }
}

impl PredisPlugin {
    fn hook_predis_execute_command(
        &self, class_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
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

                let command = execute_data.get_parameter(0).expect_mut_z_obj()?;

                let id = command.call("getid", []).context("call getId failed")?;
                let id = id.expect_z_str()?.to_str()?;

                let mut arguments = command
                    .call("getarguments", [])
                    .context("call getArguments failed")?;
                let arguments = arguments.expect_mut_z_arr()?;

                let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    Ok(ctx.create_exit_span(&format!("{}->{}", class_name, id), &peer))
                })?;

                span.with_span_object_mut(|span| {
                    span.set_span_layer(SpanLayer::Cache);
                    span.component_id = COMPONENT_PHP_PREDIS_ID;
                    span.add_tag("db.type", "redis");
                    span.add_tag("redis.command", generate_command(id, arguments));
                });

                Ok(Box::new(span))
            }),
            Box::new(move |_, span, _, return_value| {
                let mut span = span.downcast::<Span>().unwrap();

                let typ = return_value.get_type_info();
                if typ.is_null() || typ.is_false() {
                    span.with_span_object_mut(|span| span.is_error = true);
                }

                Ok(())
            }),
        )
    }
}

fn generate_command(id: &str, arguments: &mut ZArr) -> String {
    let mut ss = Vec::with_capacity(arguments.len() + 1);
    ss.push(id);

    for (_, argument) in arguments.iter() {
        if let Some(value) = argument.as_z_str().and_then(|s| s.to_str().ok()) {
            ss.push(value);
        } else if argument.as_z_arr().is_some() {
            break;
        }
    }

    ss.join(" ")
}

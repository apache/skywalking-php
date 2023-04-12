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
    component::COMPONENT_RABBITMQ_PRODUCER_ID,
    context::{RequestContext, SW_HEADER},
    execute::{get_this_mut, validate_num_args, AfterExecuteHook, BeforeExecuteHook, Noop},
    tag::{TAG_MQ_BROKER, TAG_MQ_QUEUE, TAG_MQ_TOPIC},
};
use anyhow::Context;
use phper::{
    arrays::ZArray,
    objects::ZObj,
    values::{ExecuteData, ZVal},
};
use skywalking::{skywalking_proto::v3::SpanLayer, trace::span::Span};

#[derive(Default, Clone)]
pub struct AmqplibPlugin;

impl Plugin for AmqplibPlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["PhpAmqpLib\\Channel\\AMQPChannel"])
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
                Some(class_name @ "PhpAmqpLib\\Channel\\AMQPChannel"),
                function_name @ "basic_publish",
            ) => Some(self.hook_channel_basic_publish(class_name, function_name)),
            _ => None,
        }
    }
}

impl AmqplibPlugin {
    fn hook_channel_basic_publish(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                validate_num_args(execute_data, 3)?;

                let this = get_this_mut(execute_data)?;

                let peer = Self::get_peer(this);

                let exchange = execute_data
                    .get_parameter(1)
                    .as_z_str()
                    .and_then(|s| s.to_str().ok())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| "unknown".to_owned());

                let routing_key = execute_data
                    .get_parameter(2)
                    .as_z_str()
                    .and_then(|s| s.to_str().ok())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| "unknown".to_owned());

                let span = Self::create_exit_span(
                    request_id,
                    &class_name,
                    &function_name,
                    &peer,
                    &exchange,
                    &routing_key,
                )?;

                // Self::inject_sw_header(request_id, execute_data)?;

                Ok(Box::new(span))
            }),
            Noop::noop(),
        )
    }

    fn get_peer(this: &mut ZObj) -> String {
        let Some(io) = this.get_property("connection").as_z_obj().and_then(|connection| connection.get_property("io").as_z_obj()) else {
            return "unknown:0".to_owned();
        };
        let host = io
            .get_property("host")
            .as_z_str()
            .and_then(|s| s.to_str().ok())
            .unwrap_or("unknown");
        let port = io.get_property("port").as_long().unwrap_or_default();
        format!("{}:{}", host, port)
    }

    fn create_exit_span(
        request_id: Option<i64>, class_name: &str, function_name: &str, peer: &str, exchange: &str,
        routing_key: &str,
    ) -> crate::Result<Span> {
        let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
            Ok(ctx.create_exit_span(&format!("{}->{}", class_name, function_name), peer))
        })?;

        let mut span_object = span.span_object_mut();
        span_object.set_span_layer(SpanLayer::Mq);
        span_object.component_id = COMPONENT_RABBITMQ_PRODUCER_ID;
        span_object.add_tag(TAG_MQ_BROKER, peer);
        span_object.add_tag(TAG_MQ_TOPIC, exchange);
        span_object.add_tag(TAG_MQ_QUEUE, routing_key);
        drop(span_object);

        Ok(span)
    }

    #[allow(dead_code)]
    fn inject_sw_header(
        request_id: Option<i64>, execute_data: &mut ExecuteData,
    ) -> crate::Result<()> {
        let sw_header = RequestContext::try_get_sw_header(request_id)?;

        let message = execute_data
            .get_parameter(0)
            .as_mut_z_obj()
            .context("message isn't object")?;
        let properties = message
            .get_mut_property("properties")
            .as_mut_z_arr()
            .context("message.properties isn't array")?;
        let headers = properties.get_mut("application_headers");
        match headers {
            Some(headers) => {
                if let Some(headers) = headers.as_mut_z_obj() {
                    headers.call("set", [ZVal::from(SW_HEADER), ZVal::from(sw_header)])?;
                } else if let Some(headers) = headers.as_mut_z_arr() {
                    headers.insert(SW_HEADER, sw_header);
                } else if headers.as_null().is_some() {
                    *headers = ZVal::from(Self::new_sw_headers(&sw_header));
                }
            }
            None => {
                properties.insert("application_headers", Self::new_sw_headers(&sw_header));
            }
        }

        Ok(())
    }

    fn new_sw_headers(sw_header: &str) -> ZArray {
        let mut arr = ZArray::new();
        arr.insert(SW_HEADER, sw_header);
        arr
    }
}

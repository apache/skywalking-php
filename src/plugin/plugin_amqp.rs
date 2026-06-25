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

use super::{Plugin, log_exception};
use crate::{
    component::COMPONENT_AMQP_PRODUCER_ID,
    context::{RequestContext, SW_HEADER},
    execute::{AfterExecuteHook, BeforeExecuteHook, get_this_mut, validate_num_args},
    tag::{TAG_MQ_BROKER, TAG_MQ_QUEUE, TAG_MQ_TOPIC},
};
use phper::{arrays::ZArray, objects::ZObj, values::ExecuteData};
use skywalking::{
    proto::v3::SpanLayer,
    trace::span::{HandleSpanObject, Span},
};

#[derive(Default, Clone)]
pub struct AmqpPlugin;

impl Plugin for AmqpPlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["AMQPExchange"])
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
            (Some(class_name @ "AMQPExchange"), function_name @ "publish") => {
                Some(self.hook_exchange_publish(class_name, function_name))
            }
            _ => None,
        }
    }
}

impl AmqpPlugin {
    fn hook_exchange_publish(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                validate_num_args(execute_data, 1)?;

                let this = get_this_mut(execute_data)?;

                let peer = Self::get_peer(this)?;

                let exchange = this
                    .call("getName", [])
                    .ok()
                    .and_then(|v| {
                        v.as_z_str()
                            .and_then(|s| s.to_str().ok())
                            .map(ToOwned::to_owned)
                    })
                    .unwrap_or_default();

                let routing_key = if execute_data.num_args() >= 2 {
                    execute_data
                        .get_parameter(1)
                        .as_z_str()
                        .and_then(|s| s.to_str().ok())
                        .map(ToOwned::to_owned)
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                let span = Self::create_exit_span(
                    request_id,
                    &class_name,
                    &function_name,
                    &peer,
                    &exchange,
                    &routing_key,
                )?;

                Self::inject_sw_header(request_id, execute_data, &peer)?;

                Ok(Box::new(span))
            }),
            Box::new(move |_, span, _, _| {
                let mut span = span.downcast::<Span>().unwrap();
                log_exception(&mut *span);
                Ok(())
            }),
        )
    }

    fn get_peer(this: &mut ZObj) -> crate::Result<String> {
        let mut channel = this.call("getChannel", [])?;
        let channel = channel
            .expect_mut_z_obj()
            .map_err(|e| anyhow::anyhow!("channel isn't object: {}", e))?;
        let mut connection = channel.call("getConnection", [])?;
        let connection = connection
            .expect_mut_z_obj()
            .map_err(|e| anyhow::anyhow!("connection isn't object: {}", e))?;
        let host = connection.call("getHost", [])?;
        let host = host
            .expect_z_str()
            .map_err(|e| anyhow::anyhow!("host isn't string: {}", e))?
            .to_str()?;
        let port = connection.call("getPort", [])?;
        let port = port.as_long().unwrap_or_default();
        Ok(format!("{}:{}", host, port))
    }

    fn create_exit_span(
        request_id: Option<i64>, class_name: &str, function_name: &str, peer: &str, exchange: &str,
        routing_key: &str,
    ) -> crate::Result<Span> {
        let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
            Ok(ctx.create_exit_span(&format!("{}->{}", class_name, function_name), peer))
        })?;

        let span_object = span.span_object_mut();
        span_object.set_span_layer(SpanLayer::Mq);
        span_object.component_id = COMPONENT_AMQP_PRODUCER_ID;
        span_object.add_tag(TAG_MQ_BROKER, peer);
        span_object.add_tag(TAG_MQ_TOPIC, exchange);
        span_object.add_tag(TAG_MQ_QUEUE, routing_key);

        Ok(span)
    }

    fn inject_sw_header(
        request_id: Option<i64>, execute_data: &mut ExecuteData, peer: &str,
    ) -> crate::Result<()> {
        let sw_header = RequestContext::try_get_sw_header(request_id, peer)?;

        let attributes = Self::ensure_attributes(execute_data)?;
        let headers = Self::ensure_headers(attributes)?;
        headers.insert(SW_HEADER, sw_header);

        Ok(())
    }

    fn ensure_attributes(
        execute_data: &mut ExecuteData,
    ) -> crate::Result<&mut phper::arrays::ZArr> {
        let attributes = execute_data.get_mut_parameter(3);
        if attributes.as_z_arr().is_none() {
            *attributes = ZArray::new().into();
        }

        Ok(attributes
            .as_mut_z_arr()
            .ok_or_else(|| anyhow::anyhow!("attributes isn't array"))?)
    }

    fn ensure_headers(
        attributes: &mut phper::arrays::ZArr,
    ) -> crate::Result<&mut phper::arrays::ZArr> {
        let has_headers = attributes
            .get("headers")
            .and_then(|headers| headers.as_z_arr())
            .is_some();
        if !has_headers {
            attributes.insert("headers", ZArray::new());
        }

        Ok(attributes
            .get_mut("headers")
            .and_then(|headers| headers.as_mut_z_arr())
            .ok_or_else(|| anyhow::anyhow!("headers isn't array"))?)
    }
}

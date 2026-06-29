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
    component::COMPONENT_KAFKA_PRODUCER_ID,
    context::RequestContext,
    execute::{AfterExecuteHook, BeforeExecuteHook, get_this_mut, validate_num_args},
    tag::{TAG_MQ_BROKER, TAG_MQ_QUEUE, TAG_MQ_TOPIC},
};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use phper::{objects::ZObj, sys};
use skywalking::{
    proto::v3::SpanLayer,
    trace::span::{HandleSpanObject, Span},
};

/// Maps Producer object handle -> broker string.
static PRODUCER_BROKERS: Lazy<DashMap<u32, String>> = Lazy::new(DashMap::new);

/// Maps Topic object handle -> broker string.
static TOPIC_BROKERS: Lazy<DashMap<u32, String>> = Lazy::new(DashMap::new);

/// Maps object handle -> original dtor.
static DTOR_MAP: Lazy<DashMap<u32, sys::zend_object_dtor_obj_t>> = Lazy::new(DashMap::new);

#[derive(Default, Clone)]
pub struct RdkafkaPlugin;

impl Plugin for RdkafkaPlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["RdKafka", "RdKafka\\ProducerTopic"])
    }

    fn function_name_prefix(&self) -> Option<&'static str> {
        None
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)> {
        tracing::debug!(?class_name, function_name, "rdkafka plugin hook called");
        match (class_name, function_name) {
            (Some("RdKafka"), "addBrokers") => Some(self.hook_add_brokers()),
            (Some("RdKafka"), "newTopic") => Some(self.hook_new_topic()),
            (Some("RdKafka\\ProducerTopic"), "producev") => Some(self.hook_producev()),
            _ => None,
        }
    }
}

impl RdkafkaPlugin {
    fn hook_add_brokers(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(move |_request_id, execute_data| {
                validate_num_args(execute_data, 1)?;

                let this = get_this_mut(execute_data)?;
                let handle = this.handle();
                hack_dtor(this, Some(producer_dtor));

                let broker_list = execute_data.get_parameter(0).expect_str()?.to_owned();

                PRODUCER_BROKERS.insert(handle, broker_list);

                Ok(Box::new(()))
            }),
            Box::new(|_, _, _, _| Ok(())),
        )
    }

    fn hook_new_topic(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(move |_request_id, execute_data| {
                let this = get_this_mut(execute_data)?;

                Ok(Box::new(this.handle()))
            }),
            Box::new(move |_, data, _, return_value| {
                let producer_handle = *data.downcast::<u32>().unwrap();

                if let Some(topic) = return_value.as_mut_z_obj() {
                    let class_name = topic.get_class().get_name().to_str()?;
                    if class_name == "RdKafka\\ProducerTopic" {
                        hack_dtor(topic, Some(producer_topic_dtor));
                        if let Some(broker) = PRODUCER_BROKERS.get(&producer_handle) {
                            TOPIC_BROKERS.insert(topic.handle(), broker.clone());
                        }
                    }
                }

                Ok(())
            }),
        )
    }

    fn hook_producev(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = "RdKafka\\ProducerTopic".to_owned();
        let function_name = "producev".to_owned();

        (
            Box::new(move |request_id, execute_data| {
                validate_num_args(execute_data, 1)?;

                let this = get_this_mut(execute_data)?;

                let topic_name = this
                    .call("getName", [])
                    .ok()
                    .and_then(|v| {
                        v.as_z_str()
                            .and_then(|s| s.to_str().ok())
                            .map(ToOwned::to_owned)
                    })
                    .unwrap_or_default();

                let topic_handle = this.handle();
                let broker = TOPIC_BROKERS
                    .get(&topic_handle)
                    .map(|b| b.clone())
                    .unwrap_or_default();

                let span = Self::create_exit_span(
                    request_id,
                    &class_name,
                    &function_name,
                    &broker,
                    &topic_name,
                )?;

                // TODO: rdkafka extension call parameter injection is
                // difficult, will implement later.

                Ok(Box::new(span))
            }),
            Box::new(move |_, span, _, _| {
                let mut span = span.downcast::<Span>().unwrap();
                log_exception(&mut *span);
                Ok(())
            }),
        )
    }

    fn create_exit_span(
        request_id: Option<i64>, class_name: &str, function_name: &str, peer: &str, topic: &str,
    ) -> crate::Result<Span> {
        let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
            Ok(ctx.create_exit_span(&format!("{}->{}", class_name, function_name), peer))
        })?;

        let span_object = span.span_object_mut();
        span_object.set_span_layer(SpanLayer::Mq);
        span_object.component_id = COMPONENT_KAFKA_PRODUCER_ID;
        span_object.add_tag(TAG_MQ_BROKER, peer);
        span_object.add_tag(TAG_MQ_TOPIC, topic);
        span_object.add_tag(TAG_MQ_QUEUE, "");

        Ok(span)
    }
}

fn hack_dtor(this: &mut ZObj, new_dtor: sys::zend_object_dtor_obj_t) {
    assert!(new_dtor.is_some(), "new_dtor should not be null");

    let handle = this.handle();

    unsafe {
        let ori_dtor = (*(*this.as_mut_ptr()).handlers).dtor_obj;
        DTOR_MAP.insert(handle, ori_dtor);
        (*((*this.as_mut_ptr()).handlers as *mut sys::zend_object_handlers)).dtor_obj = new_dtor;
    }
}

unsafe extern "C" fn producer_dtor(object: *mut sys::zend_object) {
    unsafe {
        let handle = ZObj::from_ptr(object).handle();
        PRODUCER_BROKERS.remove(&handle);
        if let Some((_, Some(dtor))) = DTOR_MAP.remove(&handle) {
            dtor(object);
        }
    }
}

unsafe extern "C" fn producer_topic_dtor(object: *mut sys::zend_object) {
    unsafe {
        let handle = ZObj::from_ptr(object).handle();
        TOPIC_BROKERS.remove(&handle);
        if let Some((_, Some(dtor))) = DTOR_MAP.remove(&handle) {
            dtor(object);
        }
    }
}

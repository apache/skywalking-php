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
    context::RequestContext,
    execute::{AfterExecuteHook, BeforeExecuteHook, Noop},
    log::PsrLogLevel,
    module::PSR_LOGGING_LEVEL,
};
use phper::{
    alloc::ToRefOwned, arrays::IterKey, classes::ClassEntry, functions::call, objects::ZObj,
    values::ZVal,
};
use skywalking::{
    logging::{
        logger,
        record::{LogRecord, RecordType},
    },
    trace::span::HandleSpanObject,
};
use tracing::{debug, instrument};

#[derive(Default, Clone)]
pub struct Psr3Plugin;

impl Plugin for Psr3Plugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn function_name_prefix(&self) -> Option<&'static str> {
        None
    }

    fn parent_classes(&self) -> Option<Vec<Option<&'static phper::classes::ClassEntry>>> {
        Some(vec![
            ClassEntry::from_globals(r"Psr\Log\LoggerInterface").ok()
        ])
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(
        Box<crate::execute::BeforeExecuteHook>,
        Box<crate::execute::AfterExecuteHook>,
    )> {
        let Some(class_name) = class_name else {
            return None;
        };
        match &*function_name.to_uppercase() {
            "EMERGENCY" | "ALERT" | "CRITICAL" | "ERROR" | "WARNING" | "NOTICE" | "INFO"
            | "DEBUG" => {
                let log_level = function_name.into();
                if log_level >= *PSR_LOGGING_LEVEL {
                    Some(self.hook_log_methods(
                        class_name.to_owned(),
                        function_name.to_owned(),
                        log_level,
                    ))
                } else {
                    None
                }
            }
            "LOG" => Some(self.hook_log(class_name.to_owned(), function_name.to_owned())),
            _ => None,
        }
    }
}

impl Psr3Plugin {
    #[instrument(skip_all)]
    fn hook_log_methods(
        &self, class_name: String, function_name: String, log_level: PsrLogLevel,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(move |request_id, execute_data| {
                let message = Self::handle_message(execute_data.get_mut_parameter(0))?;
                let context = Self::handle_context(execute_data.get_mut_parameter(1))?;
                Self::handle_log(
                    class_name.clone(),
                    function_name.clone(),
                    log_level.clone(),
                    request_id,
                    message,
                    context,
                )?;
                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    #[instrument(skip_all)]
    fn hook_log(
        &self, class_name: String, function_name: String,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(move |request_id, execute_data| {
                let log_level = execute_data.get_parameter(0).expect_z_str()?.to_str()?;
                let log_level: PsrLogLevel = log_level.into();
                if log_level < *PSR_LOGGING_LEVEL {
                    return Ok(Box::new(()));
                }
                let message = Self::handle_message(execute_data.get_mut_parameter(1))?;
                let context = Self::handle_context(execute_data.get_mut_parameter(2))?;
                Self::handle_log(
                    class_name.clone(),
                    function_name.clone(),
                    log_level.clone(),
                    request_id,
                    message,
                    context,
                )?;
                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn handle_log(
        class_name: String, function_name: String, log_level: PsrLogLevel, request_id: Option<i64>,
        message: String, context: Vec<(String, String)>,
    ) -> anyhow::Result<()> {
        debug!(?class_name, ?function_name, "call psr-3 log method");

        RequestContext::try_with_global(request_id, |ctx| {
            logger::log(
                LogRecord::new()
                    .record_type(RecordType::Text)
                    .content(message)
                    .with_tracing_context(&ctx.tracing_context)
                    .endpoint(&ctx.entry_span.span_object().operation_name)
                    .with_span(&ctx.entry_span)
                    .add_tag("level", log_level.to_string())
                    .add_tag("logger", &class_name)
                    .add_tags(context),
            );
            Ok(())
        })?;

        Ok(())
    }

    fn handle_message(message: &mut ZVal) -> crate::Result<String> {
        if let Some(message) = message.as_z_str() {
            Ok(message.to_str()?.to_string())
        } else if let Some(message) = message.as_mut_z_obj() {
            if let Some(message) = Self::cast_object_to_string(message)? {
                Ok(message)
            } else {
                Err("message hasn't __toString method".into())
            }
        } else {
            Err("unknown message type".into())
        }
    }

    fn handle_context(context: &mut ZVal) -> crate::Result<Vec<(String, String)>> {
        let Some(context) = context.as_mut_z_arr() else {
            return Ok(vec![]);
        };
        let mut tags = Vec::with_capacity(context.len());
        for (key, value) in context.iter_mut() {
            match key {
                IterKey::Index(_) => continue,
                IterKey::ZStr(key) => {
                    let key = key.to_str()?.to_string();

                    let value = if value.as_null().is_some() {
                        "null".to_string()
                    } else if let Some(value) = value.as_bool() {
                        value.to_string()
                    } else if let Some(value) = value.as_long() {
                        value.to_string()
                    } else if let Some(value) = value.as_double() {
                        value.to_string()
                    } else if let Some(value) = value.as_z_str() {
                        value.to_str()?.to_string()
                    } else if value.as_z_arr().is_some() {
                        "Array".to_string()
                    } else if let Some(value) = value.as_mut_z_obj() {
                        let Some(value) = Self::cast_object_to_string(value)? else {
                            continue;
                        };
                        value
                    } else {
                        continue;
                    };

                    tags.push((key, value));
                }
            }
        }
        Ok(tags)
    }

    fn cast_object_to_string(obj: &mut ZObj) -> crate::Result<Option<String>> {
        if call(
            "method_exists",
            [obj.to_ref_owned().into(), "__toString".into()],
        )?
        .as_bool()
            == Some(true)
        {
            let s = obj.call("__toString", [])?;
            Ok(Some(s.expect_z_str()?.to_str()?.to_string()))
        } else {
            Ok(None)
        }
    }
}

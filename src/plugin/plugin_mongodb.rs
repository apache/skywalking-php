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

use super::{log_exception, Plugin};
use crate::{
    component::COMPONENT_MONGODB_ID,
    context::RequestContext,
    execute::{get_this_mut, AfterExecuteHook, BeforeExecuteHook},
    tag::TAG_DB_TYPE,
};
use phper::{
    objects::ZObj,
    values::{ExecuteData, ZVal},
};
use skywalking::{
    proto::v3::SpanLayer,
    trace::span::{HandleSpanObject, Span},
};
use std::any::Any;
use tracing::{debug, error};
use crate::module::TOKEN_NAME;

const MANAGER_CLASS_NAME: &str = r"MongoDB\Driver\Manager";

#[derive(Default, Clone)]
pub struct MongodbPlugin;

impl Plugin for MongodbPlugin {
    #[inline]
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&[MANAGER_CLASS_NAME])
    }

    #[inline]
    fn function_name_prefix(&self) -> Option<&'static str> {
        None
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)> {
        match (class_name, function_name) {
            (Some(MANAGER_CLASS_NAME), f)
                if ["executebulkwrite", "executequery"].contains(&&*f.to_ascii_lowercase()) =>
            {
                Some(self.hook_manager_execute_namespace_method(function_name))
            }
            (Some(MANAGER_CLASS_NAME), f)
                if [
                    "executecommand",
                    "executereadcommand",
                    "executereadwritecommand",
                    "executewritecommand",
                ]
                .contains(&&*f.to_ascii_lowercase()) =>
            {
                Some(self.hook_manager_execute_db_method(function_name))
            }
            _ => None,
        }
    }
}

impl MongodbPlugin {
    fn hook_manager_execute_namespace_method(
        &self, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                before_manager_crud_hook(
                    request_id,
                    execute_data,
                    &function_name,
                    CrudScope::Namespace,
                )
            }),
            Box::new(after_manager_crud_hook),
        )
    }

    fn hook_manager_execute_db_method(
        &self, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                before_manager_crud_hook(request_id, execute_data, &function_name, CrudScope::Db)
            }),
            Box::new(after_manager_crud_hook),
        )
    }
}

enum CrudScope {
    Namespace,
    Db,
}

fn before_manager_crud_hook(
    request_id: Option<i64>, execute_data: &mut ExecuteData, function_name: &str, scope: CrudScope,
) -> crate::Result<Box<dyn Any>> {
    let this = get_this_mut(execute_data)?;
    let handle = this.handle();
    debug!(handle, function_name, "call MongoDB Manager CRUD method");

    let mut span = RequestContext::try_with_global_ctx(request_id, |ctx| {
        // Since the driver connects to the database lazily, peer here is empty and
        // reset it in after hook.
        Ok(ctx.create_exit_span(&format!("{}->{}", MANAGER_CLASS_NAME, function_name), ""))
    })?;

    let span_object = span.span_object_mut();
    span_object.set_span_layer(SpanLayer::Database);
    span_object.add_tag("token", &*TOKEN_NAME);
    span_object.component_id = COMPONENT_MONGODB_ID;
    span_object.add_tag(TAG_DB_TYPE, "MongoDB");

    if let Some(id) = execute_data
        .get_parameter(0)
        .as_z_str()
        .and_then(|s| s.to_str().ok())
    {
        match scope {
            CrudScope::Namespace => {
                let mut segments = id.split('.');
                if let Some(db) = segments.next() {
                    span_object.add_tag("mongo.db", db);
                }
                if let Some(collection) = segments.next() {
                    span_object.add_tag("mongo.collection", collection);
                }
            }
            CrudScope::Db => {
                span_object.add_tag("mongo.db", id);
            }
        }
    }

    Ok(Box::new(span))
}

fn after_manager_crud_hook(
    _: Option<i64>, span: Box<dyn Any>, execute_data: &mut ExecuteData, _return_value: &mut ZVal,
) -> crate::Result<()> {
    let mut span = span.downcast::<Span>().unwrap();

    let this = get_this_mut(execute_data)?;
    let peer = match get_peer(this) {
        Ok(peer) => peer,
        Err(err) => {
            error!(?err, "get peer failed");
            "".to_string()
        }
    };
    span.span_object_mut().peer = peer;

    log_exception(&mut *span);

    Ok(())
}

fn get_peer(this: &mut ZObj) -> phper::Result<String> {
    let mut addr = Vec::new();

    let mut servers = this.call("getServers", [])?;
    let servers = servers.expect_mut_z_arr()?;

    for (_, server) in servers.iter_mut() {
        let server = server.expect_mut_z_obj()?;

        let host = server.call("getHost", [])?;
        let host = host.expect_z_str()?.to_str()?;

        let port = server.call("getPort", [])?.expect_long()?;

        addr.push(format!("{}:{}", host, port));
    }

    Ok(addr.join(";"))
}

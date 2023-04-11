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

use anyhow::anyhow;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use skywalking::trace::{
    propagation::encoder::encode_propagation, span::Span, trace_context::TracingContext,
};

pub const SW_HEADER: &str = "sw8";

static REQUEST_CONTEXT: Lazy<DashMap<Option<i64>, RequestContext>> = Lazy::new(DashMap::new);

pub struct RequestContext {
    pub tracing_context: TracingContext,
    pub entry_span: Span,
}

impl RequestContext {
    pub fn set_global(request_id: Option<i64>, ctx: Self) {
        REQUEST_CONTEXT.insert(request_id, ctx);
    }

    pub fn remove_global(request_id: Option<i64>) -> Option<Self> {
        REQUEST_CONTEXT.remove(&request_id).map(|(_, ctx)| ctx)
    }

    pub fn try_with_global<T>(
        request_id: Option<i64>, f: impl FnOnce(&mut RequestContext) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        REQUEST_CONTEXT
            .get_mut(&request_id)
            .map(|mut ctx| f(ctx.value_mut()))
            .transpose()?
            .ok_or_else(|| anyhow!("global tracing context not exists"))
    }

    pub fn try_with_global_ctx<T>(
        request_id: Option<i64>, f: impl FnOnce(&mut TracingContext) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        Self::try_with_global(request_id, |ctx| f(&mut ctx.tracing_context))
    }

    pub fn try_get_sw_header(request_id: Option<i64>) -> crate::Result<String> {
        Ok(Self::try_with_global(request_id, |req_ctx| {
            let span_object = req_ctx.get_primary_span().span_object();
            Ok(encode_propagation(
                &req_ctx.tracing_context,
                &span_object.operation_name,
                &span_object.peer,
            ))
        })?)
    }

    /// Primary endpoint name is used for endpoint dependency.
    fn get_primary_span(&self) -> &Span {
        &self.entry_span
    }
}

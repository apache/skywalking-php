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
    component::COMPONENT_PHP_PDO_ID,
    context::RequestContext,
    execute::{get_this_mut, validate_num_args, AfterExecuteHook, BeforeExecuteHook, Noop},
    tag::{TAG_DB_STATEMENT, TAG_DB_TYPE},
};
use anyhow::Context;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use phper::{
    arrays::ZArr,
    objects::ZObj,
    sys,
    values::{ExecuteData, ZVal},
};
use skywalking::{skywalking_proto::v3::SpanLayer, trace::span::Span};
use std::{any::Any, str::FromStr};
use tracing::{debug, warn};

static DSN_MAP: Lazy<DashMap<u32, Dsn>> = Lazy::new(Default::default);
static DTOR_MAP: Lazy<DashMap<u32, sys::zend_object_dtor_obj_t>> = Lazy::new(Default::default);

#[derive(Default, Clone)]
pub struct PdoPlugin;

impl Plugin for PdoPlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["PDO", "PDOStatement"])
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
            (Some("PDO"), "__construct") => Some(self.hook_pdo_construct()),
            (Some("PDO"), f)
                if [
                    "exec",
                    "query",
                    "prepare",
                    "commit",
                    "begintransaction",
                    "rollback",
                ]
                .contains(&f) =>
            {
                Some(self.hook_pdo_methods(function_name))
            }
            (Some("PDOStatement"), f)
                if ["execute", "fetch", "fetchAll", "fetchColumn", "fetchObject"].contains(&f) =>
            {
                Some(self.hook_pdo_statement_methods(function_name))
            }
            _ => None,
        }
    }
}

impl PdoPlugin {
    fn hook_pdo_construct(&self) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        (
            Box::new(|_, execute_data| {
                validate_num_args(execute_data, 1)?;

                let this = get_this_mut(execute_data)?;
                let handle = this.handle();
                hack_dtor(this, Some(pdo_dtor));

                let dsn = execute_data.get_parameter(0);
                let dsn = dsn.as_z_str().context("dsn isn't str")?.to_str()?;
                debug!(dsn, "construct PDO");

                let dsn: Dsn = dsn.parse()?;
                debug!(?dsn, "parse PDO dsn");

                DSN_MAP.insert(handle, dsn);

                Ok(Box::new(()))
            }),
            Noop::noop(),
        )
    }

    fn hook_pdo_methods(
        &self, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let handle = get_this_mut(execute_data)?.handle();

                debug!(handle, function_name, "call PDO method");

                let mut span = with_dsn(handle, |dsn| {
                    create_exit_span_with_dsn(request_id, "PDO", &function_name, dsn)
                })?;

                if execute_data.num_args() >= 1 {
                    if let Some(statement) = execute_data.get_parameter(0).as_z_str() {
                        span.add_tag(TAG_DB_STATEMENT, statement.to_str()?);
                    }
                }

                Ok(Box::new(span) as _)
            }),
            Box::new(after_hook),
        )
    }

    fn hook_pdo_statement_methods(
        &self, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let this = get_this_mut(execute_data)?;
                let handle = this.handle();

                debug!(handle, function_name, "call PDOStatement method");

                let mut span = with_dsn(handle, |dsn| {
                    create_exit_span_with_dsn(request_id, "PDOStatement", &function_name, dsn)
                })?;

                if let Some(query) = this.get_property("queryString").as_z_str() {
                    span.add_tag(TAG_DB_STATEMENT, query.to_str()?);
                } else {
                    warn!("PDOStatement queryString is empty");
                }

                Ok(Box::new(span) as _)
            }),
            Box::new(after_hook),
        )
    }
}

fn hack_dtor(this: &mut ZObj, new_dtor: sys::zend_object_dtor_obj_t) {
    let handle = this.handle();

    unsafe {
        let ori_dtor = (*(*this.as_mut_ptr()).handlers).dtor_obj;
        DTOR_MAP.insert(handle, ori_dtor);
        (*((*this.as_mut_ptr()).handlers as *mut sys::zend_object_handlers)).dtor_obj = new_dtor;
    }
}

unsafe extern "C" fn pdo_dtor(object: *mut sys::zend_object) {
    debug!("call PDO dtor");
    dtor(object);
}

unsafe extern "C" fn pdo_statement_dtor(object: *mut sys::zend_object) {
    debug!("call PDOStatement dtor");
    dtor(object);
}

unsafe extern "C" fn dtor(object: *mut sys::zend_object) {
    let handle = ZObj::from_ptr(object).handle();

    DSN_MAP.remove(&handle);
    if let Some((_, Some(dtor))) = DTOR_MAP.remove(&handle) {
        dtor(object);
    }
}

fn after_hook(
    _: Option<i64>, span: Box<dyn Any>, execute_data: &mut ExecuteData, return_value: &mut ZVal,
) -> crate::Result<()> {
    if let Some(b) = return_value.as_bool() {
        if !b {
            return after_hook_when_false(
                get_this_mut(execute_data)?,
                &mut span.downcast::<Span>().unwrap(),
            );
        }
    } else if let Some(obj) = return_value.as_mut_z_obj() {
        if obj.get_class().get_name() == &"PDOStatement" {
            return after_hook_when_pdo_statement(get_this_mut(execute_data)?, obj);
        }
    }

    Ok(())
}

fn after_hook_when_false(this: &mut ZObj, span: &mut Span) -> crate::Result<()> {
    let info = this.call("errorInfo", [])?;
    let info = info.as_z_arr().context("errorInfo isn't array")?;

    let state = get_error_info_item(info, 0)?.expect_z_str()?.to_str()?;
    let code = {
        let code = get_error_info_item(info, 1)?;
        // PDOStatement::fetch
        // In all cases, false is returned on failure or if there are no more rows.
        if code.get_type_info().is_null() {
            return Ok(());
        }

        &code.expect_long()?.to_string()
    };
    let error = get_error_info_item(info, 2)?.expect_z_str()?.to_str()?;

    let mut span_object = span.span_object_mut();
    span_object.is_error = true;
    span_object.add_log([("SQLSTATE", state), ("Error Code", code), ("Error", error)]);

    Ok(())
}

fn after_hook_when_pdo_statement(pdo: &mut ZObj, pdo_statement: &mut ZObj) -> crate::Result<()> {
    let dsn = DSN_MAP
        .get(&pdo.handle())
        .map(|r| r.value().clone())
        .context("DSN not found")?;
    DSN_MAP.insert(pdo_statement.handle(), dsn);
    hack_dtor(pdo_statement, Some(pdo_statement_dtor));
    Ok(())
}

fn get_error_info_item(info: &ZArr, i: u64) -> anyhow::Result<&ZVal> {
    info.get(i)
        .with_context(|| format!("errorInfo[{}] not exists", i))
}

fn create_exit_span_with_dsn(
    request_id: Option<i64>, class_name: &str, function_name: &str, dsn: &Dsn,
) -> anyhow::Result<Span> {
    RequestContext::try_with_global_ctx(request_id, |ctx| {
        let mut span =
            ctx.create_exit_span(&format!("{}->{}", class_name, function_name), &dsn.peer);

        let mut span_object = span.span_object_mut();
        span_object.set_span_layer(SpanLayer::Database);
        span_object.component_id = COMPONENT_PHP_PDO_ID;
        span_object.add_tag(TAG_DB_TYPE, &dsn.db_type);
        span_object.add_tag("db.data_source", &dsn.data_source);
        drop(span_object);

        Ok(span)
    })
}

fn with_dsn<T>(handle: u32, f: impl FnOnce(&Dsn) -> anyhow::Result<T>) -> anyhow::Result<T> {
    DSN_MAP
        .get(&handle)
        .map(|r| f(r.value()))
        .context("dns not exists")?
}

#[derive(Debug, Clone)]
struct Dsn {
    db_type: String,
    data_source: String,
    peer: String,
}

impl FromStr for Dsn {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ss = s.splitn(2, ':');
        let db_type = ss.next().context("unknown db type")?.to_owned();
        let data_source = ss.next().context("unknown datasource")?.to_owned();

        let mut host = "unknown";
        let mut port = match &*db_type {
            "mysql" => "3306",
            "oci" => "1521", // Oracle
            "sqlsrv" => "1433",
            "pgsql" => "5432",
            _ => "0",
        };

        let ss = data_source.split(';');
        for s in ss {
            if s.is_empty() {
                continue;
            }

            let mut kv = s.splitn(2, '=');
            let k = kv.next().context("unknown key")?;
            let v = kv.next().context("unknown value")?;

            // TODO compact the fields rather than mysql.
            match k {
                "host" => {
                    host = v;
                }
                "port" => {
                    port = v;
                }
                _ => {}
            }
        }

        let peer = if host.contains(':') {
            host.to_string()
        } else {
            host.to_string() + ":" + port
        };

        Ok(Dsn {
            db_type,
            data_source,
            peer,
        })
    }
}

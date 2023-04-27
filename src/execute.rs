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

use crate::{
    plugin::select_plugin,
    request::{HACK_SWOOLE_ON_REQUEST_FUNCTION_NAME, IS_SWOOLE},
    util::catch_unwind_result,
};
use anyhow::{bail, Context};
use phper::{
    objects::ZObj,
    strings::ZStr,
    sys,
    values::{ExecuteData, ZVal},
};
use std::{any::Any, panic::AssertUnwindSafe, ptr::null_mut, sync::atomic::Ordering};
use tracing::{error, trace};

pub type BeforeExecuteHook =
    dyn FnOnce(Option<i64>, &mut ExecuteData) -> crate::Result<Box<dyn Any>>;

pub type AfterExecuteHook =
    dyn FnOnce(Option<i64>, Box<dyn Any>, &mut ExecuteData, &mut ZVal) -> crate::Result<()>;

pub trait Noop {
    fn noop() -> Self;
}

impl Noop for Box<BeforeExecuteHook> {
    #[inline]
    fn noop() -> Self {
        fn f(_: Option<i64>, _: &mut ExecuteData) -> crate::Result<Box<dyn Any>> {
            Ok(Box::new(()))
        }
        Box::new(f)
    }
}

impl Noop for Box<AfterExecuteHook> {
    #[inline]
    fn noop() -> Self {
        fn f(
            _: Option<i64>, _: Box<dyn Any>, _: &mut ExecuteData, _: &mut ZVal,
        ) -> crate::Result<()> {
            Ok(())
        }
        Box::new(f)
    }
}

static mut ORI_EXECUTE_INTERNAL: Option<
    unsafe extern "C" fn(execute_data: *mut sys::zend_execute_data, return_value: *mut sys::zval),
> = None;

static mut ORI_EXECUTE_EX: Option<unsafe extern "C" fn(execute_data: *mut sys::zend_execute_data)> =
    None;

unsafe extern "C" fn execute_internal(
    execute_data: *mut sys::zend_execute_data, return_value: *mut sys::zval,
) {
    let (execute_data, return_value) = match (
        ExecuteData::try_from_mut_ptr(execute_data),
        ZVal::try_from_mut_ptr(return_value),
    ) {
        (Some(execute_data), Some(return_value)) => (execute_data, return_value),
        (execute_data, return_value) => {
            ori_execute_internal(execute_data, return_value);
            return;
        }
    };

    let (function_name, class_name) = match get_function_and_class_name(execute_data) {
        Ok(x) => x,
        Err(err) => {
            error!(?err, "get function and class name failed");
            ori_execute_internal(Some(execute_data), Some(return_value));
            return;
        }
    };

    trace!(
        ?function_name,
        ?class_name,
        "execute_internal function and class name"
    );

    let function_name = match function_name {
        Some(function_name) => function_name,
        None => {
            ori_execute_internal(Some(execute_data), Some(return_value));
            return;
        }
    };

    if function_name == HACK_SWOOLE_ON_REQUEST_FUNCTION_NAME {
        ori_execute_internal(Some(execute_data), Some(return_value));
        return;
    }

    let plugin = select_plugin(class_name.as_deref(), &function_name);
    let plugin = match plugin {
        Some(plugin) => plugin,
        None => {
            ori_execute_internal(Some(execute_data), Some(return_value));
            return;
        }
    };

    let (before, after) = match plugin.hook(class_name.as_deref(), &function_name) {
        Some(hook) => hook,
        None => {
            ori_execute_internal(Some(execute_data), Some(return_value));
            return;
        }
    };

    let request_id = infer_request_id(execute_data);
    trace!(
        ?request_id,
        ?function_name,
        ?class_name,
        "execute_internal infer request id"
    );

    let result = catch_unwind_result(AssertUnwindSafe(|| before(request_id, execute_data)));
    if let Err(err) = &result {
        error!(?err, "before execute internal");
    }

    ori_execute_internal(Some(execute_data), Some(return_value));

    // If before hook return error, don't execute the after hook.
    if let Ok(data) = result {
        if let Err(err) = catch_unwind_result(AssertUnwindSafe(|| {
            after(request_id, data, execute_data, return_value)
        })) {
            error!(?err, "after execute internal");
        }
    }
}

unsafe extern "C" fn execute_ex(execute_data: *mut sys::zend_execute_data) {
    let execute_data = match ExecuteData::try_from_mut_ptr(execute_data) {
        Some(execute_data) => execute_data,
        None => {
            ori_execute_ex(None);
            return;
        }
    };

    let (function_name, class_name) = match get_function_and_class_name(execute_data) {
        Ok(x) => x,
        Err(err) => {
            error!(?err, "get function and class name failed");
            ori_execute_ex(Some(execute_data));
            return;
        }
    };

    trace!(
        ?function_name,
        ?class_name,
        "execute_ex function and class name"
    );

    let function_name = match function_name {
        Some(function_name) => function_name,
        None => {
            ori_execute_ex(Some(execute_data));
            return;
        }
    };

    let plugin = select_plugin(class_name.as_deref(), &function_name);
    let plugin = match plugin {
        Some(plugin) => plugin,
        None => {
            ori_execute_ex(Some(execute_data));
            return;
        }
    };

    let (before, after) = match plugin.hook(class_name.as_deref(), &function_name) {
        Some(hook) => hook,
        None => {
            ori_execute_ex(Some(execute_data));
            return;
        }
    };

    let request_id = infer_request_id(execute_data);
    trace!(
        ?request_id,
        ?function_name,
        ?class_name,
        "execute_ex infer request id"
    );

    let result = catch_unwind_result(AssertUnwindSafe(|| before(request_id, execute_data)));
    if let Err(err) = &result {
        error!(?err, "before execute ex");
    }

    ori_execute_ex(Some(execute_data));

    // If before hook return error, don't execute the after hook.
    if let Ok(data) = result {
        let mut null = ZVal::from(());
        let return_value = match ZVal::try_from_mut_ptr((*execute_data.as_mut_ptr()).return_value) {
            Some(return_value) => return_value,
            None => &mut null,
        };
        if let Err(err) = catch_unwind_result(AssertUnwindSafe(|| {
            after(request_id, data, execute_data, return_value)
        })) {
            error!(?err, "after execute ex");
        }
    }
}

#[inline]
fn ori_execute_internal(execute_data: Option<&mut ExecuteData>, return_value: Option<&mut ZVal>) {
    let execute_data = execute_data
        .map(ExecuteData::as_mut_ptr)
        .unwrap_or(null_mut());
    let return_value = return_value.map(ZVal::as_mut_ptr).unwrap_or(null_mut());
    unsafe {
        match ORI_EXECUTE_INTERNAL {
            Some(f) => f(execute_data, return_value),
            None => sys::execute_internal(execute_data, return_value),
        }
    }
}

#[inline]
fn ori_execute_ex(execute_data: Option<&mut ExecuteData>) {
    unsafe {
        if let Some(f) = ORI_EXECUTE_EX {
            f(execute_data
                .map(ExecuteData::as_mut_ptr)
                .unwrap_or(null_mut()))
        }
    }
}

pub fn register_execute_functions() {
    unsafe {
        ORI_EXECUTE_INTERNAL = sys::zend_execute_internal;
        sys::zend_execute_internal = Some(execute_internal);

        ORI_EXECUTE_EX = sys::zend_execute_ex;
        sys::zend_execute_ex = Some(execute_ex);
    }
}

pub fn validate_num_args(execute_data: &mut ExecuteData, num: usize) -> anyhow::Result<()> {
    if execute_data.num_args() < num {
        bail!("argument count incorrect");
    }
    Ok(())
}

pub fn get_this_mut(execute_data: &mut ExecuteData) -> anyhow::Result<&mut ZObj> {
    execute_data.get_this_mut().context("$this is empty")
}

fn get_function_and_class_name(
    execute_data: &mut ExecuteData,
) -> anyhow::Result<(Option<String>, Option<String>)> {
    let function = execute_data.func();

    let function_name = function
        .get_function_name()
        .map(ZStr::to_str)
        .transpose()?
        .map(ToOwned::to_owned);
    let class_name = function
        .get_class()
        .map(|cls| cls.get_name().to_str().map(ToOwned::to_owned))
        .transpose()?;

    Ok((function_name, class_name))
}

fn infer_request_id(execute_data: &mut ExecuteData) -> Option<i64> {
    if !IS_SWOOLE.load(Ordering::Relaxed) {
        return None;
    }

    let mut prev_execute_data_ptr = execute_data.as_mut_ptr();
    loop {
        let Some(prev_execute_data) = (unsafe { ExecuteData::try_from_mut_ptr(prev_execute_data_ptr) }) else {
            return None;
        };
        let func_name = prev_execute_data.func().get_function_name();
        if !func_name
            .map(|s| s == &HACK_SWOOLE_ON_REQUEST_FUNCTION_NAME.as_bytes())
            .unwrap_or_default()
        {
            prev_execute_data_ptr = unsafe { (*prev_execute_data_ptr).prev_execute_data };
            continue;
        }
        let Some(request) = prev_execute_data.get_mut_parameter(0).as_mut_z_obj() else {
            return None;
        };
        match request.get_mut_property("fd").as_long() {
            Some(fd) => return Some(fd),
            None => {
                error!("infer request id failed");
                return None;
            }
        }
    }
}

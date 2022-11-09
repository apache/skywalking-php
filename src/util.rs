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

use anyhow::bail;
use chrono::Local;
use once_cell::sync::Lazy;
use phper::{arrays::IterKey, sys, values::ZVal};
use serde_json::{json, Number, Value};
use std::{
    ffi::CStr,
    panic::{catch_unwind, UnwindSafe},
};
use systemstat::{IpAddr, Platform, System};

pub static IPS: Lazy<Vec<String>> = Lazy::new(|| {
    System::new()
        .networks()
        .ok()
        .and_then(|networks| {
            let addrs = networks
                .values()
                .flat_map(|network| {
                    network
                        .addrs
                        .iter()
                        .filter_map(|network_addr| match network_addr.addr {
                            IpAddr::V4(addr) => {
                                if network.name == "lo"
                                    || network.name.starts_with("docker")
                                    || network.name.starts_with("br-")
                                {
                                    None
                                } else {
                                    Some(addr.to_string())
                                }
                            }
                            _ => None,
                        })
                })
                .collect::<Vec<_>>();

            if addrs.is_empty() {
                None
            } else {
                Some(addrs)
            }
        })
        .unwrap_or_else(|| vec!["127.0.0.1".to_owned()])
});

// TODO Maybe report_instance_properties used.
#[allow(dead_code)]
pub static HOST_NAME: Lazy<String> = Lazy::new(|| {
    hostname::get()
        .ok()
        .and_then(|hostname| hostname.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
});

// TODO Maybe report_instance_properties used.
#[allow(dead_code)]
pub const OS_NAME: &str = if cfg!(target_os = "linux") {
    "Linux"
} else if cfg!(target_os = "windows") {
    "Windows"
} else if cfg!(target_os = "macos") {
    "Macos"
} else {
    "Unknown"
};

// TODO Maybe report_instance_properties used.
#[allow(dead_code)]
pub fn current_formatted_time() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn z_val_to_string(zv: &ZVal) -> Option<String> {
    zv.as_z_str()
        .and_then(|zs| zs.to_str().ok())
        .map(|s| s.to_string())
}

pub fn catch_unwind_anyhow<F: FnOnce() -> anyhow::Result<R> + UnwindSafe, R>(
    f: F,
) -> anyhow::Result<R> {
    match catch_unwind(f) {
        Ok(r) => r,
        Err(e) => {
            if let Some(s) = e.downcast_ref::<&str>() {
                bail!("paniced: {}", s);
            } else if let Some(s) = e.downcast_ref::<String>() {
                bail!("paniced: {}", s);
            } else {
                bail!("paniced");
            }
        }
    }
}

pub fn get_sapi_module_name() -> &'static CStr {
    unsafe { CStr::from_ptr(sys::sapi_module.name) }
}

/// Use for later scene.
#[allow(dead_code)]
pub fn json_encode_values(values: &[ZVal]) -> serde_json::Result<String> {
    fn add(json_value: &mut Value, key: Option<String>, item: Value) {
        match key {
            Some(key) => {
                json_value.as_object_mut().unwrap().insert(key, item);
            }
            None => {
                json_value.as_array_mut().unwrap().push(item);
            }
        }
    }

    fn handle(json_value: &mut Value, key: Option<String>, val: &ZVal) {
        let type_info = val.get_type_info();

        if type_info.is_null() {
            add(json_value, key, Value::Null);
        } else if type_info.is_true() {
            add(json_value, key, Value::Bool(true));
        } else if type_info.is_false() {
            add(json_value, key, Value::Bool(false));
        } else if type_info.is_long() {
            let i = val.as_long().unwrap();
            add(json_value, key, Value::Number(i.into()));
        } else if type_info.is_double() {
            let d = val.as_double().unwrap();
            let n = match Number::from_f64(d) {
                Some(n) => Value::Number(n),
                None => Value::String("<NaN>".to_owned()),
            };
            add(json_value, key, n);
        } else if type_info.is_string() {
            let s = val
                .as_z_str()
                .unwrap()
                .to_str()
                .map(ToOwned::to_owned)
                .unwrap_or_default();
            add(json_value, key, Value::String(s));
        } else if type_info.is_array() {
            let arr = val.as_z_arr().unwrap();
            let is_arr = arr.iter().all(|(key, _)| matches!(key, IterKey::Index(_)));
            let mut new_json_value = if is_arr { json!([]) } else { json!({}) };
            for (key, new_val) in arr.iter() {
                if is_arr {
                    handle(&mut new_json_value, None, new_val);
                } else {
                    let key = match key {
                        IterKey::Index(i) => i.to_string(),
                        IterKey::ZStr(s) => s.to_str().map(ToOwned::to_owned).unwrap_or_default(),
                    };
                    handle(&mut new_json_value, Some(key), new_val);
                }
            }
            add(json_value, key, new_json_value);
        } else if type_info.is_object() {
            add(json_value, key, Value::String("<Object>".to_owned()));
        }
    }

    let mut json_value = json!([]);
    for val in values {
        handle(&mut json_value, None, val);
    }
    serde_json::to_string(&json_value)
}

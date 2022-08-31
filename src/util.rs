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
use phper::{sys, values::ZVal};
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

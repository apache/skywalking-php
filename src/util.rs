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
use once_cell::sync::Lazy;
use phper::{sys, values::ZVal};
use std::{
    ffi::CStr,
    os::unix::prelude::OsStrExt,
    panic::{catch_unwind, UnwindSafe},
    path::Path,
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

pub fn z_val_to_string(zv: &ZVal) -> Option<String> {
    zv.as_z_str()
        .and_then(|zs| zs.to_str().ok())
        .map(|s| s.to_string())
}

pub fn catch_unwind_result<F: FnOnce() -> crate::Result<R> + UnwindSafe, R>(
    f: F,
) -> crate::Result<R> {
    match catch_unwind(f) {
        Ok(r) => r,
        Err(e) => {
            if let Some(s) = e.downcast_ref::<&str>() {
                Err(anyhow!("paniced: {}", s).into())
            } else if let Some(s) = e.downcast_ref::<String>() {
                Err(anyhow!("paniced: {}", s).into())
            } else {
                Err(anyhow!("paniced").into())
            }
        }
    }
}

pub fn get_sapi_module_name() -> &'static CStr {
    unsafe { CStr::from_ptr(sys::sapi_module.name) }
}

pub fn change_permission(f: impl AsRef<Path>, mode: libc::mode_t) {
    let f = f.as_ref().as_os_str().as_bytes();
    let mut path = Vec::with_capacity(f.len() + 1);
    path.extend_from_slice(f);
    path.push(b'\0');
    unsafe {
        libc::chmod(path.as_ptr().cast(), mode);
    }
}

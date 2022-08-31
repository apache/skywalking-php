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

use ipc_channel::ipc::IpcSharedMemory;
use once_cell::sync::Lazy;
use std::{
    mem::{size_of, transmute},
    sync::atomic::AtomicUsize,
};

#[derive(Default)]
pub struct Global {
    pub channel_size: AtomicUsize,
    // pub worker_pid: AtomicU32,
}

/// Global shared memory.
pub static GLOBAL: Lazy<&Global> = Lazy::new(|| {
    static SHARE: Lazy<IpcSharedMemory> = Lazy::new(|| {
        let buf: [u8; size_of::<Global>()] = unsafe { transmute(Global::default()) };
        IpcSharedMemory::from_bytes(&buf)
    });
    let share: &[u8] = &SHARE;
    unsafe { (share.as_ptr() as *const Global).as_ref().unwrap() }
});

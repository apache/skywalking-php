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

mod plugin_amqplib;
mod plugin_curl;
mod plugin_memcached;
mod plugin_mysqli;
mod plugin_pdo;
mod plugin_predis;
mod plugin_redis;
mod plugin_swoole;

use crate::execute::{AfterExecuteHook, BeforeExecuteHook};
use once_cell::sync::Lazy;

// Register plugins here.
static PLUGINS: Lazy<Vec<Box<DynPlugin>>> = Lazy::new(|| {
    vec![
        Box::<plugin_curl::CurlPlugin>::default(),
        Box::<plugin_pdo::PdoPlugin>::default(),
        Box::<plugin_mysqli::MySQLImprovedPlugin>::default(),
        Box::<plugin_swoole::SwooleServerPlugin>::default(),
        Box::<plugin_swoole::SwooleHttpResponsePlugin>::default(),
        Box::<plugin_predis::PredisPlugin>::default(),
        Box::<plugin_memcached::MemcachedPlugin>::default(),
        Box::<plugin_redis::RedisPlugin>::default(),
        Box::<plugin_amqplib::AmqplibPlugin>::default(),
    ]
});

pub type DynPlugin = dyn Plugin + Send + Sync + 'static;

pub trait Plugin {
    fn class_names(&self) -> Option<&'static [&'static str]>;

    fn function_name_prefix(&self) -> Option<&'static str>;

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)>;
}

pub fn select_plugin(class_name: Option<&str>, function_name: &str) -> Option<&'static DynPlugin> {
    let mut selected_plugin = None;

    for plugin in &*PLUGINS {
        if let Some(class_name) = class_name {
            if let Some(plugin_class_names) = plugin.class_names() {
                if plugin_class_names.contains(&class_name) {
                    selected_plugin = Some(plugin);
                    break;
                }
            }
        }
        if let Some(function_name_prefix) = plugin.function_name_prefix() {
            if function_name.starts_with(function_name_prefix) {
                selected_plugin = Some(plugin);
                break;
            }
        }
    }

    selected_plugin.map(AsRef::as_ref)
}

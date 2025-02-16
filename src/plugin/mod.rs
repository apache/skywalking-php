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
mod plugin_memcache;
mod plugin_memcached;
mod plugin_mongodb;
mod plugin_mysqli;
mod plugin_pdo;
mod plugin_predis;
mod plugin_psr3;
mod plugin_redis;
mod plugin_swoole;
mod style;

use crate::{
    execute::{AfterExecuteHook, BeforeExecuteHook},
    log::PsrLogLevel,
    module::PSR_LOGGING_LEVEL,
};
use once_cell::sync::Lazy;
use phper::{classes::ClassEntry, eg, objects::ZObj};
use skywalking::trace::span::HandleSpanObject;
use std::{collections::HashMap, ops::Deref, sync::Mutex};
use tracing::error;

// Register plugins here.
static PLUGINS: Lazy<Vec<Box<DynPlugin>>> = Lazy::new(|| {
    let mut plugins: Vec<Box<DynPlugin>> = vec![
        Box::<plugin_curl::CurlPlugin>::default(),
        Box::<plugin_pdo::PdoPlugin>::default(),
        Box::<plugin_mysqli::MySQLImprovedPlugin>::default(),
        Box::<plugin_swoole::SwooleServerPlugin>::default(),
        Box::<plugin_swoole::SwooleHttpResponsePlugin>::default(),
        Box::<plugin_predis::PredisPlugin>::default(),
        Box::<plugin_memcached::MemcachedPlugin>::default(),
        Box::<plugin_redis::RedisPlugin>::default(),
        Box::<plugin_amqplib::AmqplibPlugin>::default(),
        Box::<plugin_mongodb::MongodbPlugin>::default(),
        Box::<plugin_memcache::MemcachePlugin>::default(),
    ];
    if *PSR_LOGGING_LEVEL > PsrLogLevel::Off {
        plugins.push(Box::<plugin_psr3::Psr3Plugin>::default());
    }
    plugins
});

pub type DynPlugin = dyn Plugin + Send + Sync + 'static;

pub trait Plugin {
    fn class_names(&self) -> Option<&'static [&'static str]>;

    fn function_name_prefix(&self) -> Option<&'static str>;

    fn parent_classes(&self) -> Option<Vec<Option<&'static ClassEntry>>> {
        None
    }

    fn hook(
        &self, class_name: Option<&str>, function_name: &str,
    ) -> Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)>;
}

pub fn select_plugin_hook(
    class_name: Option<&str>, function_name: &str,
) -> Option<(&'static BeforeExecuteHook, &'static AfterExecuteHook)> {
    type HookMap =
        HashMap<(Option<String>, String), Option<(Box<BeforeExecuteHook>, Box<AfterExecuteHook>)>>;

    static LOCK: Lazy<Mutex<()>> = Lazy::new(Default::default);
    static mut HOOK_MAP: Lazy<HookMap> = Lazy::new(HashMap::new);

    let _guard = match LOCK.lock() {
        Ok(guard) => guard,
        Err(err) => {
            error!(?err, "get lock failed");
            return None;
        }
    };
    unsafe {
        HOOK_MAP
            .entry((class_name.map(ToOwned::to_owned), function_name.to_owned()))
            .or_insert_with(|| {
                select_plugin(class_name, function_name)
                    .and_then(|plugin| plugin.hook(class_name, function_name))
            })
            .as_ref()
            .map(|(before, after)| (before.deref(), after.deref()))
    }
}

fn select_plugin(class_name: Option<&str>, function_name: &str) -> Option<&'static DynPlugin> {
    let mut selected_plugin = None;

    'plugin: for plugin in &*PLUGINS {
        if let Some(class_name) = class_name {
            if let Some(plugin_class_names) = plugin.class_names() {
                if plugin_class_names.contains(&class_name) {
                    selected_plugin = Some(plugin);
                    break 'plugin;
                }
            }
            if let Some(parent_classes) = plugin.parent_classes() {
                if let Ok(class) = ClassEntry::from_globals(class_name) {
                    // Iterate parent_classes and skip None.
                    for parent_class in parent_classes.into_iter().flatten() {
                        if class.is_instance_of(parent_class) {
                            selected_plugin = Some(plugin);
                            break 'plugin;
                        }
                    }
                }
            }
        }
        if let Some(function_name_prefix) = plugin.function_name_prefix() {
            if function_name.starts_with(function_name_prefix) {
                selected_plugin = Some(plugin);
                break 'plugin;
            }
        }
    }

    selected_plugin.map(AsRef::as_ref)
}

fn log_exception(span: &mut impl HandleSpanObject) -> Option<&mut ZObj> {
    let mut ex = unsafe { ZObj::try_from_mut_ptr(eg!(exception)) };
    if let Some(ex) = ex.as_mut() {
        let span_object = span.span_object_mut();
        span_object.is_error = true;

        let mut logs = Vec::new();
        if let Ok(class_name) = ex.get_class().get_name().to_str() {
            logs.push(("error.kind", class_name.to_owned()));
        }
        if let Some(message) = ex.get_property("message").as_z_str() {
            if let Ok(message) = message.to_str() {
                logs.push(("message", message.to_owned()));
            }
        }
        if let Ok(stack) = ex.call("getTraceAsString", []) {
            if let Some(stack) = stack.as_z_str().and_then(|s| s.to_str().ok()) {
                logs.push(("stack", stack.to_owned()));
            }
        }
        if !logs.is_empty() {
            span_object.add_log(logs);
        }
    }
    ex
}

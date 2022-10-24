use super::Plugin;
use crate::{
    component::COMPONENT_PHP_MEMCACHED_ID,
    context::RequestContext,
    execute::{get_this_mut, AfterExecuteHook, BeforeExecuteHook, Noop},
};
use anyhow::{bail, Context};
use once_cell::sync::Lazy;
use phper::{functions::call, values::ExecuteData};
use skywalking::skywalking_proto::v3::SpanLayer;
use tracing::warn;

// TODO Detect is error.

static MEC_KEYS_COMMANDS: Lazy<Vec<String>> = Lazy::new(|| {
    [
        "set",
        "setByKey",
        "setMulti",
        "setMultiByKey",
        "add",
        "addByKey",
        "replace",
        "replaceByKey",
        "append",
        "appendByKey",
        "prepend",
        "prependByKey",
        "get",
        "getByKey",
        "getMulti",
        "getMultiByKey",
        "getAllKeys",
        "delete",
        "deleteByKey",
        "deleteMulti",
        "deleteMultiByKey",
        "increment",
        "incrementByKey",
        "decrement",
        "decrementByKey",
        "getStats",
        "isPersistent",
        "isPristine",
        "flush",
        "flushBuffers",
        "getDelayed",
        "getDelayedByKey",
        "fetch",
        "fetchAll",
        "addServer",
        "addServers",
        "getOption",
        "setOption",
        "setOptions",
        "getResultCode",
        "getServerList",
        "resetServerList",
        "getVersion",
        "quit",
        "setSaslAuthData",
        "touch",
        "touchByKey",
    ]
    .into_iter()
    .map(str::to_ascii_lowercase)
    .collect()
});

static MEC_STR_KEYS_COMMANDS: Lazy<Vec<String>> = Lazy::new(|| {
    [
        "set",
        "setByKey",
        "setMulti",
        "setMultiByKey",
        "add",
        "addByKey",
        "replace",
        "replaceByKey",
        "append",
        "appendByKey",
        "prepend",
        "prependByKey",
        "get",
        "getByKey",
        "getMulti",
        "getMultiByKey",
        "getAllKeys",
        "delete",
        "deleteByKey",
        "deleteMulti",
        "deleteMultiByKey",
        "increment",
        "incrementByKey",
        "decrement",
        "decrementByKey",
    ]
    .into_iter()
    .map(str::to_ascii_lowercase)
    .collect()
});

#[derive(Default, Clone)]
pub struct MemcachedPlugin;

impl Plugin for MemcachedPlugin {
    fn class_names(&self) -> Option<&'static [&'static str]> {
        Some(&["Memcached"])
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
            (Some(class_name @ "Memcached"), f)
                if MEC_KEYS_COMMANDS.contains(&f.to_ascii_lowercase()) =>
            {
                Some(self.hook_memcached_methods(class_name, function_name))
            }
            _ => None,
        }
    }
}

impl MemcachedPlugin {
    fn hook_memcached_methods(
        &self, class_name: &str, function_name: &str,
    ) -> (Box<BeforeExecuteHook>, Box<AfterExecuteHook>) {
        let class_name = class_name.to_owned();
        let function_name = function_name.to_owned();
        (
            Box::new(move |request_id, execute_data| {
                let peer = if MEC_STR_KEYS_COMMANDS.contains(&function_name.to_ascii_lowercase()) {
                    let mut f = || {
                        let key = {
                            let key = execute_data.get_parameter(0);
                            if !key.get_type_info().is_string() {
                                bail!("The argument key of {} isn't string", &function_name);
                            }
                            key.clone()
                        };
                        let this = get_this_mut(execute_data)?;
                        let info = this.call(&"getServerByKey".to_ascii_lowercase(), [key])?;
                        let info = info.as_z_arr().context("Server isn't array")?;
                        let host = info
                            .get("host")
                            .context("Server host not exists")?
                            .as_z_str()
                            .context("Server host isn't string")?
                            .to_str()?;
                        let port = info
                            .get("port")
                            .context("Server port not exists")?
                            .as_long()
                            .context("Server port isn't long")?;
                        Ok::<_, anyhow::Error>(format!("{}:{}", host, port))
                    };
                    match f() {
                        Ok(peer) => peer,
                        Err(err) => {
                            warn!(?err, "Get peer failed");
                            "".to_owned()
                        }
                    }
                } else {
                    "".to_owned()
                };

                let span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    let mut span =
                        ctx.create_exit_span(&format!("{}->{}", class_name, function_name), &peer);
                    span.with_span_object_mut(|obj| {
                        obj.set_span_layer(SpanLayer::Cache);
                        obj.component_id = COMPONENT_PHP_MEMCACHED_ID;
                        obj.add_tag("db.type", "memcached");

                        match get_command(execute_data, &function_name) {
                            Ok(cmd) => {
                                obj.add_tag("memcached.command", cmd);
                            }
                            Err(err) => {
                                warn!(?err, "get command failed");
                            }
                        }
                    });
                    Ok(span)
                });

                Ok(Box::new(span) as _)
            }),
            Noop::noop(),
        )
    }
}

fn get_command(execute_data: &mut ExecuteData, function_name: &str) -> anyhow::Result<String> {
    let num_args = execute_data.num_args();
    let mut items = Vec::with_capacity(num_args + 1);
    items.push(function_name.to_owned());

    for i in 0..num_args {
        let parameter = execute_data.get_parameter(i);
        let s = if parameter.get_type_info().is_array() {
            let result = call("json_encode", [parameter.clone()])?;
            result.expect_z_str()?.to_str()?.to_string()
        } else {
            let mut parameter = parameter.clone();
            parameter.convert_to_string();
            parameter.expect_z_str()?.to_str()?.to_string()
        };
        items.push(s)
    }

    Ok(items.join(" "))
}

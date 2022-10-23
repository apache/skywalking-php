use super::Plugin;
use crate::{
    component::COMPONENT_PHP_MEMCACHED_ID,
    context::RequestContext,
    execute::{get_this_mut, AfterExecuteHook, BeforeExecuteHook, Noop},
};
use once_cell::sync::Lazy;
use skywalking::skywalking_proto::v3::SpanLayer;

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
                let mut peer = "".to_owned();
                if MEC_STR_KEYS_COMMANDS.contains(&function_name.to_ascii_lowercase()) {
                    execute_data.get_parameter(0);
                }

                let span = RequestContext::try_with_global_ctx(request_id, |ctx| {
                    let mut span =
                        ctx.create_exit_span(&format!("{}->{}", class_name, function_name), peer);
                    span.with_span_object_mut(|obj| {
                        obj.set_span_layer(SpanLayer::Cache);
                        obj.component_id = COMPONENT_PHP_MEMCACHED_ID;
                        obj.add_tag("db.type", "memcached");
                    });
                    Ok(span)
                });

                // let handle = get_this_mut(execute_data)?.handle();

                // debug!(handle, function_name, "call PDO method");

                // let mut span = with_dsn(handle, |dsn| {
                //     create_exit_span_with_dsn("PDO", &function_name, dsn)
                // })?;

                // if execute_data.num_args() >= 1 {
                //     if let Some(statement) = execute_data.get_parameter(0).as_z_str() {
                //         span.add_tag("db.statement", statement.to_str()?);
                //     }
                // }

                Ok(Box::new(span) as _)
            }),
            Noop::noop(),
        )
    }
}

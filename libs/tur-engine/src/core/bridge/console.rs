use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::property::PropertyDescriptor;
use boa_engine::Context;
use boa_engine::JsValue;

fn format_args(args: &[JsValue]) -> String {
    args.iter()
        .map(|v| v.display().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn register_console_globals(context: &mut Context) {
    let log = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            tracing::info!("{}", format_args(args));
            Ok(JsValue::undefined())
        })
    };
    let warn = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            tracing::warn!("{}", format_args(args));
            Ok(JsValue::undefined())
        })
    };
    let error = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            tracing::error!("{}", format_args(args));
            Ok(JsValue::undefined())
        })
    };
    let info = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            tracing::info!("{}", format_args(args));
            Ok(JsValue::undefined())
        })
    };
    let debug = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx| {
            tracing::debug!("{}", format_args(args));
            Ok(JsValue::undefined())
        })
    };

    let proto = context.intrinsics().constructors().object().prototype();
    let console = boa_engine::object::JsObject::from_proto_and_data(proto, ());
    let set = |name: &str, f: NativeFunction, obj: &boa_engine::object::JsObject| {
        let func = boa_engine::object::FunctionObjectBuilder::new(
            context.realm(),
            f,
        )
        .name(js_string!(name))
        .build();
        let desc = PropertyDescriptor::builder()
            .value(func)
            .writable(true)
            .enumerable(false)
            .configurable(true)
            .build();
        obj.insert_property(js_string!(name), desc);
    };

    set("log", log, &console);
    set("warn", warn, &console);
    set("error", error, &console);
    set("info", info, &console);
    set("debug", debug, &console);    context
        .register_global_property(js_string!("console"), console, boa_engine::property::Attribute::all())
        .expect("failed to register console");
}

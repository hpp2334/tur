use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::property::PropertyDescriptor;
use boa_engine::Context;
use boa_engine::JsResult;
use boa_engine::JsValue;

fn format_args(args: &[JsValue], ctx: &mut Context) -> String {
    args.iter()
        .map(|v| {
            let mut out = v.display().to_string();
            if let Some(obj) = v.as_object() {
                if let Ok(msg) = obj.get(js_string!("message"), ctx) {
                    if let Some(s) = msg.as_string() {
                        out.push('\n');
                        out.push_str(&s.to_std_string_escaped());
                    }
                }
                if let Ok(stack) = obj.get(js_string!("stack"), ctx) {
                    if let Some(s) = stack.as_string() {
                        out.push('\n');
                        out.push_str(&s.to_std_string_escaped());
                    }
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

macro_rules! console_fn {
    ($name:ident, $level:ident) => {
        fn $name(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
            tracing::$level!("{}", format_args(args, ctx));
            Ok(JsValue::undefined())
        }
    };
}

console_fn!(console_log, info);
console_fn!(console_warn, warn);
console_fn!(console_error, error);
console_fn!(console_info, info);
console_fn!(console_debug, debug);

pub fn register_console_globals(context: &mut Context) {
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

    set("log", NativeFunction::from_fn_ptr(console_log), &console);
    set("warn", NativeFunction::from_fn_ptr(console_warn), &console);
    set("error", NativeFunction::from_fn_ptr(console_error), &console);
    set("info", NativeFunction::from_fn_ptr(console_info), &console);
    set("debug", NativeFunction::from_fn_ptr(console_debug), &console);
    context
        .register_global_property(js_string!("console"), console, boa_engine::property::Attribute::all())
        .expect("failed to register console");
}

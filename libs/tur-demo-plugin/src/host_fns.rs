//! Host service functions: swc-backed compiler services exposed to JS as
//! `transpileTsx`, `tokenizeTsx`, `generateAst`. ctx-free `NativeFunction`s
//! registered by `TurDemoPlugin` as part of the `builtin:demo-helper` module.

use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsArray;
use boa_engine::object::JsObject;
use boa_engine::{js_string, JsArgs, JsError, JsNativeError, JsValue};

use crate::compiler;

/// Build the swc-backed compiler host functions:
/// - `transpileTsx(src): string` (throws on parse error)
/// - `tokenizeTsx(src): Array<{ start, end, kind }>` (lexical token categories
///   refined by AST-derived semantic categories — declaration names, JSX
///   tags/attributes, type names, comments — for syntax highlighting)
/// - `generateAst(src): AstNode[]`
pub fn build_host_service_fns() -> Vec<(&'static str, boa_engine::NativeFunction, usize)> {
    let transpile = NativeFunction::from_copy_closure(|_this, args, _ctx| {
        let src = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message("transpileTsx: expected a string"))
            })?
            .to_std_string_escaped();
        match compiler::transpile_tsx(&src) {
            Ok(code) => Ok(JsValue::from(js_string!(code))),
            Err(e) => Err(JsError::from(JsNativeError::typ().with_message(e))),
        }
    });

    let tokenize = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let src = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message("tokenizeTsx: expected a string"))
            })?
            .to_std_string_escaped();
        let spans = compiler::highlight_tsx(&src);
        let arr = JsArray::new(ctx)?;
        for sp in spans {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            obj.create_data_property(js_string!("start"), JsValue::from(sp.start as f64), ctx)?;
            obj.create_data_property(js_string!("end"), JsValue::from(sp.end as f64), ctx)?;
            obj.create_data_property(js_string!("kind"), JsValue::from(sp.kind as f64), ctx)?;
            arr.push(obj, ctx)?;
        }
        Ok(arr.into())
    });

    let generate_ast = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let src = args
            .get_or_undefined(0)
            .as_string()
            .ok_or_else(|| {
                JsError::from(JsNativeError::typ().with_message("generateAst: expected a string"))
            })?
            .to_std_string_escaped();
        let nodes = compiler::generate_ast(&src)
            .map_err(|e| JsError::from(JsNativeError::typ().with_message(e)))?;

        let arr = JsArray::new(ctx)?;
        for node in nodes {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            let kind_str = match &node.kind {
                compiler::AstNodeKind::Import { .. } => "import",
                compiler::AstNodeKind::ExportDecl { .. } => "exportDecl",
                compiler::AstNodeKind::ExportDefault => "exportDefault",
                compiler::AstNodeKind::ExportNamed { .. } => "exportNamed",
                compiler::AstNodeKind::ExportAll => "exportAll",
                compiler::AstNodeKind::ExportType { .. } => "exportType",
                compiler::AstNodeKind::Statement => "statement",
            };
            obj.create_data_property(js_string!("kind"), JsValue::from(js_string!(kind_str)), ctx)?;
            obj.create_data_property(js_string!("text"), JsValue::from(js_string!(node.text.as_str())), ctx)?;
            if let Some(body) = &node.body {
                obj.create_data_property(js_string!("body"), JsValue::from(js_string!(body.as_str())), ctx)?;
            }

            match &node.kind {
                compiler::AstNodeKind::Import { source, specifiers } => {
                    obj.create_data_property(js_string!("source"), JsValue::from(js_string!(source.as_str())), ctx)?;
                    let spec_arr = JsArray::new(ctx)?;
                    for spec in specifiers {
                        let spec_obj = JsObject::with_object_proto(ctx.intrinsics());
                        spec_obj.create_data_property(js_string!("local"), JsValue::from(js_string!(spec.local.as_str())), ctx)?;
                        spec_obj.create_data_property(js_string!("imported"), JsValue::from(js_string!(spec.imported.as_str())), ctx)?;
                        spec_arr.push(spec_obj, ctx)?;
                    }
                    obj.create_data_property(js_string!("specifiers"), JsValue::from(spec_arr), ctx)?;
                }
                compiler::AstNodeKind::ExportDecl { names }
                | compiler::AstNodeKind::ExportNamed { names }
                | compiler::AstNodeKind::ExportType { names } => {
                    let name_arr = JsArray::new(ctx)?;
                    for n in names {
                        name_arr.push(JsValue::from(js_string!(n.as_str())), ctx)?;
                    }
                    obj.create_data_property(js_string!("names"), JsValue::from(name_arr), ctx)?;
                }
                _ => {}
            }

            arr.push(obj, ctx)?;
        }
        Ok(arr.into())
    });

    vec![
        ("transpileTsx", transpile, 1),
        ("tokenizeTsx", tokenize, 1),
        ("generateAst", generate_ast, 1),
    ]
}

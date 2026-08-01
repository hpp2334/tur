//! UTF-8 text encoding/decoding utilities — merged into `tur:std`.
//!
//! Exposes:
//! - `decodeUtf8(bytes: Uint8Array | ArrayBuffer): string`
//! - `encodeUtf8(text: string): Uint8Array`
//!
//! These are needed because boa does not implement the Web Platform
//! `TextDecoder` / `TextEncoder` APIs. The event-bus payload model (raw
//! bytes) relies on these helpers to round-trip between JS strings and
//! `Uint8Array`.

use boa_engine::object::builtins::{JsArrayBuffer, JsTypedArray, JsUint8Array};
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue, js_string};

use crate::core::js_runtime::helpers::{FnEntry, Ptr};
use crate::error::TurError;

pub fn install_encode() -> Result<Vec<FnEntry>, TurError> {
    Ok(vec![
        ("decodeUtf8", 1, tur_decode_utf8 as Ptr),
        ("encodeUtf8", 1, tur_encode_utf8 as Ptr),
    ])
}

fn tur_decode_utf8(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let bytes = extract_bytes(args, ctx)?;
    let text = String::from_utf8(bytes).map_err(|e| {
        JsError::from(JsNativeError::typ().with_message(format!("invalid UTF-8: {e}")))
    })?;
    Ok(JsValue::from(js_string!(text)))
}

fn tur_encode_utf8(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let text = args
        .get_or_undefined(1)
        .as_string()
        .ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("encodeUtf8: expected a string"))
        })?
        .to_std_string_escaped();
    let bytes = text.into_bytes();
    let u8a = JsUint8Array::from_iter(bytes, ctx)?;
    Ok(JsValue::from(u8a))
}

fn extract_bytes(args: &[JsValue], ctx: &mut Context) -> JsResult<Vec<u8>> {
    let v = args.get_or_undefined(1);
    let obj = v.as_object().ok_or_else(|| {
        JsError::from(
            JsNativeError::typ().with_message("decodeUtf8: expected Uint8Array or ArrayBuffer"),
        )
    })?;
    if let Ok(ta) = JsTypedArray::from_object(obj.clone()) {
        let offset = ta.byte_offset(ctx).unwrap_or(0);
        let len = ta.byte_length(ctx).unwrap_or(0);
        let buf_val = ta.buffer(ctx)?;
        let ab = JsArrayBuffer::from_object(buf_val.as_object().unwrap().clone())?;
        let full = ab.to_vec().unwrap_or_default();
        return Ok(full[offset..offset + len].to_vec());
    }
    if let Ok(ab) = JsArrayBuffer::from_object(obj.clone()) {
        return Ok(ab.to_vec().unwrap_or_default());
    }
    Err(JsError::from(JsNativeError::typ().with_message(
        "decodeUtf8: expected Uint8Array or ArrayBuffer",
    )))
}

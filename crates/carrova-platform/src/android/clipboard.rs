//! The clipboard through `Clipboard.kt` (`ClipboardManager`).

use ::jni::JNIEnv;
use ::jni::objects::{JByteArray, JObject, JObjectArray, JString};

use super::{clip_kind, is_sensitive, jni};
use crate::{ClipContent, ClipKind, PlatformError};

/// JNI type of the glue's `Clipboard.Clip`.
const CLIP: &str = "Lorg/byteveda/carrova/platform/Clipboard$Clip;";

/// The primary clip, if it holds text or a PNG. Android hands it only to
/// the app in the foreground.
pub(super) fn read() -> Result<Option<ClipContent>, PlatformError> {
    jni::call("Clipboard.read", |env, context| {
        let sig = format!("(Landroid/content/Context;){CLIP}");
        let clip =
            jni::call_static(env, context, "Clipboard", "read", &sig, &[context.into()])?.l()?;
        if clip.is_null() {
            return Ok(None);
        }
        let mimes = env.get_field(&clip, "mimes", "[Ljava/lang/String;")?.l()?;
        let mimes = strings(env, &mimes.into())?;
        let Some(kind) = clip_kind(mimes.iter().map(String::as_str)) else {
            return Ok(None);
        };
        let bytes = env.get_field(&clip, "bytes", "[B")?.l()?;
        if bytes.is_null() {
            return Ok(None);
        }
        let bytes = env.convert_byte_array(JByteArray::from(bytes))?;
        let extra = env
            .get_field(&clip, "sensitive", "Ljava/lang/String;")?
            .l()?;
        let extra = optional_string(env, extra)?;
        Ok(Some(ClipContent {
            kind,
            bytes,
            sensitive: is_sensitive(extra.as_deref()),
        }))
    })
}

/// Replace the primary clip, marked sensitive if `content` is.
pub(super) fn write(content: &ClipContent) -> Result<(), PlatformError> {
    let mime = match content.kind {
        ClipKind::Text => "text/plain",
        ClipKind::Png => "image/png",
    };
    jni::call("Clipboard.write", |env, context| {
        let mime = env.new_string(mime)?;
        let bytes = env.byte_array_from_slice(&content.bytes)?;
        jni::call_static(
            env,
            context,
            "Clipboard",
            "write",
            "(Landroid/content/Context;Ljava/lang/String;[BZ)V",
            &[
                context.into(),
                (&mime).into(),
                (&bytes).into(),
                content.sensitive.into(),
            ],
        )?
        .v()
    })
}

fn strings(env: &mut JNIEnv<'_>, array: &JObjectArray<'_>) -> ::jni::errors::Result<Vec<String>> {
    let len = env.get_array_length(array)?;
    (0..len)
        .filter_map(|i| {
            env.get_object_array_element(array, i)
                .and_then(|s| optional_string(env, s))
                .transpose()
        })
        .collect()
}

fn optional_string(env: &mut JNIEnv<'_>, s: JObject<'_>) -> ::jni::errors::Result<Option<String>> {
    if s.is_null() {
        return Ok(None);
    }
    Ok(Some(env.get_string(&JString::from(s))?.into()))
}

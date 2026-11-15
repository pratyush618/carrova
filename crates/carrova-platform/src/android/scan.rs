//! QR scanning through `Scanner.kt` (ZXing, offline): the camera screen
//! opens over the app and the call waits for its result.

use ::jni::objects::JString;

use super::jni;
use crate::PlatformError;

/// The scanned text, or `None` if the user cancelled (or it timed out).
pub(super) fn scan_qr() -> Result<Option<String>, PlatformError> {
    jni::call("Scanner.scan", |env, context| {
        let text = jni::call_static(
            env,
            context,
            "Scanner",
            "scan",
            "(Landroid/content/Context;)Ljava/lang/String;",
            &[context.into()],
        )?
        .l()?;
        if text.is_null() {
            return Ok(None);
        }
        Ok(Some(env.get_string(&JString::from(text))?.into()))
    })
}

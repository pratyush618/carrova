//! The JNI bridge to the Kotlin glue (`org.byteveda.carrova.platform.*`).
//!
//! The one module of the workspace allowed `unsafe` (ADR 0003): turning the
//! raw JavaVM, context and file descriptor handed over by Android into the
//! `jni` crate's safe types. Everything it exposes is safe.

// ADR 0003: the only allow-listed `unsafe` in the workspace.
#![allow(unsafe_code)]

use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, RawFd};
use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JObject, JValue, JValueOwned};
use jni::{JNIEnv, JavaVM};

use crate::PlatformError;

/// Package of the Kotlin glue classes.
const GLUE_PACKAGE: &str = "org.byteveda.carrova.platform";
/// Local references one call may hold before they are freed together.
const LOCAL_FRAME: i32 = 32;

/// JavaVM and application context handed over by `LinkService` when it
/// starts Rust without an activity (ADR 0003, lifetime mechanism 2).
static SERVICE_CONTEXT: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();

/// Keep the JavaVM and application `context` of a process Rust was started
/// in without an activity, so the platform calls work there too. Only the
/// first call counts. Not `ndk_context::initialize_android_context`: tao
/// asserts that one is unset when the activity opens later.
///
/// # Errors
///
/// [`PlatformError::Android`] if the JavaVM or a global reference to
/// `context` cannot be had.
pub fn attach_service_context(
    env: &JNIEnv<'_>,
    context: &JObject<'_>,
) -> Result<(), PlatformError> {
    let failed = |e: jni::errors::Error| PlatformError::Android {
        call: "attach_service_context",
        cause: e.to_string(),
    };
    let vm = env.get_java_vm().map_err(failed)?;
    let context = env.new_global_ref(context).map_err(failed)?;
    let _ = SERVICE_CONTEXT.set((vm, context));
    Ok(())
}

/// Run `f` on this thread (attached to the JavaVM for good) with the
/// application context, freeing its local references afterwards. A Java
/// exception is cleared and reported as [`PlatformError::Android`] for
/// `call` (exception class only: messages may hold names or URLs).
pub(super) fn call<T>(
    call: &'static str,
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> jni::errors::Result<T>,
) -> Result<T, PlatformError> {
    let failed = |cause: String| PlatformError::Android { call, cause };
    let (vm, context) = app_context().map_err(|e| failed(e.to_string()))?;
    let mut env = vm
        .attach_current_thread_permanently()
        .map_err(|e| failed(e.to_string()))?;
    // SAFETY: `context` is a live global reference: tao's (never deleted)
    // or the one kept in `SERVICE_CONTEXT` for the life of the process.
    // `JObject` does not delete what it wraps.
    let context = unsafe { JObject::from_raw(context) };
    env.with_local_frame(LOCAL_FRAME, |env| {
        f(env, &context).map_err(|e| Failure(exception_or(env, &e)))
    })
    .map_err(|Failure(cause)| failed(cause))
}

/// Call the static `method` (JNI signature `sig`) of glue class `class`.
/// Loaded through the context's class loader: a thread Rust attached sees
/// only the system classes through `FindClass`.
pub(super) fn call_static<'l>(
    env: &mut JNIEnv<'l>,
    context: &JObject<'_>,
    class: &str,
    method: &str,
    sig: &str,
    args: &[JValue<'_, '_>],
) -> jni::errors::Result<JValueOwned<'l>> {
    let loader = env
        .call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;
    let name = env.new_string(format!("{GLUE_PACKAGE}.{class}"))?;
    let class = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[(&name).into()],
        )?
        .l()?;
    env.call_static_method(JClass::from(class), method, sig, args)
}

/// Take ownership of `fd`, a descriptor the glue detached for Rust
/// (`ParcelFileDescriptor.detachFd`).
pub(super) fn file_from_fd(fd: RawFd) -> io::Result<File> {
    if fd < 0 {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    // SAFETY: the glue detached `fd` from its `ParcelFileDescriptor`, so
    // nothing else owns or closes it; it is open (checked non-negative,
    // fresh from `detachFd`).
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// The JavaVM and the raw application context: the service's if it
/// started Rust, else the one tao set from the activity.
fn app_context() -> jni::errors::Result<(JavaVM, jni::sys::jobject)> {
    if let Some((vm, context)) = SERVICE_CONTEXT.get() {
        // SAFETY: the pointer comes from a live `JavaVM`, valid for the
        // life of the process.
        let vm = unsafe { JavaVM::from_raw(vm.get_java_vm_pointer()) }?;
        return Ok((vm, context.as_obj().as_raw()));
    }
    // Set by tao in the activity's `onCreate`, before Tauri's `setup` (the
    // only other caller); panics if neither ran, which cannot happen.
    let ndk = ndk_context::android_context();
    // SAFETY: tao stored the process's JavaVM pointer, valid for its life.
    let vm = unsafe { JavaVM::from_raw(ndk.vm().cast()) }?;
    Ok((vm, ndk.context().cast()))
}

/// Why a call failed, carried out of the local frame.
struct Failure(String);

impl From<jni::errors::Error> for Failure {
    fn from(e: jni::errors::Error) -> Self {
        Self(e.to_string())
    }
}

/// The class of the pending Java exception (cleared), else `e`.
fn exception_or(env: &mut JNIEnv<'_>, e: &jni::errors::Error) -> String {
    if !env.exception_check().unwrap_or(false) {
        return e.to_string();
    }
    let thrown = env.exception_occurred();
    let _ = env.exception_clear();
    thrown
        .and_then(|thrown| {
            let class = env.get_object_class(&thrown)?;
            let name = env
                .call_method(&class, "getName", "()Ljava/lang/String;", &[])?
                .l()?;
            Ok(env.get_string(&name.into())?.into())
        })
        .unwrap_or_else(|_| e.to_string())
}

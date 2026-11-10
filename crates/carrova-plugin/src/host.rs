//! Host functions (import module `carrova`), each linked only when the
//! manifest declares it; a module importing anything else is refused at
//! load.
//!
//! | Import | Type | Declared by |
//! |--------|------|-------------|
//! | `log` | `(ptr, len: i32)` | `log: true` |
//! | `run_tool` | `(name_ptr, name_len, in_ptr, in_len: i32) -> i64` | `run_tools` |
//!
//! Any misuse (an undeclared tool, a buffer outside memory, a failing
//! tool) traps: the plugin's call fails with [`crate::PluginError::Trap`].

use carrova_platform::Platform;
use wasmi::{Caller, Engine, Extern, Linker, Memory, StoreLimits};

use crate::{MAX_IO_BYTES, PluginError, PluginManifest, RunBudget, tool};

/// Import module of the host functions.
const MODULE: &str = "carrova";
/// Longest log line kept from one `log` call, in bytes; the rest is cut.
pub const MAX_LOG_LINE: usize = 1024;

/// Store data of one call: memory limits and what host functions reach.
pub(crate) struct Host<'a> {
    pub(crate) limits: StoreLimits,
    pub(crate) manifest: &'a PluginManifest,
    /// `None` while loading: tools cannot run then.
    pub(crate) platform: Option<&'a dyn Platform>,
    pub(crate) budget: &'a mut RunBudget,
}

/// Whether `manifest` lets its module import `module.name`.
pub(crate) fn allowed(manifest: &PluginManifest, module: &str, name: &str) -> bool {
    module == MODULE
        && match name {
            "log" => manifest.log,
            "run_tool" => !manifest.run_tools.is_empty(),
            _ => false,
        }
}

/// A linker with exactly the host functions `manifest` declares.
pub(crate) fn linker<'a>(
    engine: &Engine,
    manifest: &PluginManifest,
) -> Result<Linker<Host<'a>>, PluginError> {
    let mut linker = Linker::new(engine);
    let failed = |e: wasmi::errors::LinkerError| PluginError::Module(e.to_string());
    if manifest.log {
        linker.func_wrap(MODULE, "log", log).map_err(failed)?;
    }
    if !manifest.run_tools.is_empty() {
        linker
            .func_wrap(MODULE, "run_tool", run_tool)
            .map_err(failed)?;
    }
    Ok(linker)
}

fn trap(why: impl Into<String>) -> wasmi::Error {
    wasmi::Error::new(why.into())
}

/// `carrova.log`: one line to Carrova's log at debug level, target
/// `plugin`, tagged with the plugin id. Plugins see file contents, so a
/// line may hold some: it is cut to [`MAX_LOG_LINE`], stripped of control
/// and bidi characters and counted against the run's
/// [`crate::MAX_RUN_LOG`].
fn log(mut caller: Caller<'_, Host<'_>>, ptr: i32, len: i32) -> Result<(), wasmi::Error> {
    let len = usize::try_from(len).map_err(|_| trap("log: negative length"))?;
    let bytes = read(&caller, ptr, len.min(MAX_LOG_LINE))?;
    let mut line: String = String::from_utf8_lossy(&bytes)
        .chars()
        .map(|c| {
            if carrova_manifest::is_unsafe_char(c) {
                char::REPLACEMENT_CHARACTER
            } else {
                c
            }
        })
        .collect();
    if len > MAX_LOG_LINE {
        line.push('…');
    }
    let host = caller.data_mut();
    if let Some(line) = host.budget.take_log(line) {
        tracing::debug!(target: "plugin", plugin = %host.manifest.id, "{line}");
    }
    Ok(())
}

/// `carrova.run_tool`: run a tool the manifest declares (see
/// [`crate::tool`]) with the input buffer as stdin; returns
/// `(out_ptr << 32) | out_len` of its stdout, copied into a buffer from the
/// plugin's own `alloc`.
fn run_tool(
    mut caller: Caller<'_, Host<'_>>,
    name_ptr: i32,
    name_len: i32,
    in_ptr: i32,
    in_len: i32,
) -> Result<i64, wasmi::Error> {
    let (manifest, platform) = (caller.data().manifest, caller.data().platform);
    let undeclared = || trap("run_tool: tool not declared in plugin.yaml");
    let name_len = usize::try_from(name_len).map_err(|_| undeclared())?;
    if name_len > crate::manifest::MAX_TOOL_NAME {
        return Err(undeclared());
    }
    let name = read(&caller, name_ptr, name_len)?;
    let name = manifest
        .run_tools
        .iter()
        .find(|t| t.as_bytes() == name)
        .ok_or_else(undeclared)?;
    let in_len = usize::try_from(in_len)
        .ok()
        .filter(|&n| n as u64 <= MAX_IO_BYTES)
        .ok_or_else(|| trap("run_tool: input too large"))?;
    let input = read(&caller, in_ptr, in_len)?;
    let platform = platform.ok_or_else(|| trap("run_tool: tools cannot run while loading"))?;
    let out = tool::run(platform, name, input, caller.data_mut().budget).map_err(trap)?;
    let len = i32::try_from(out.len()).map_err(|_| trap("run_tool: output too large"))?;
    let alloc = caller
        .get_export("alloc")
        .and_then(Extern::into_func)
        .ok_or_else(|| trap("run_tool: no `alloc`"))?
        .typed::<i32, i32>(&caller)?;
    let ptr = alloc.call(&mut caller, len)?;
    memory(&caller)?
        .write(&mut caller, ptr as u32 as usize, &out)
        .map_err(|_| trap("run_tool: `alloc` returned a buffer outside memory"))?;
    Ok((i64::from(ptr as u32) << 32) | i64::from(len as u32))
}

fn memory(caller: &Caller<'_, Host<'_>>) -> Result<Memory, wasmi::Error> {
    caller
        .get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(|| trap("no `memory` export"))
}

/// `len` bytes at `ptr` in the plugin's memory (bounds-checked).
fn read(caller: &Caller<'_, Host<'_>>, ptr: i32, len: usize) -> Result<Vec<u8>, wasmi::Error> {
    let mut buf = vec![0; len];
    memory(caller)?
        .read(caller, ptr as u32 as usize, &mut buf)
        .map_err(|_| trap("buffer outside the plugin's memory"))?;
    Ok(buf)
}

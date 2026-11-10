//! [`Plugin`]: a validated module and the per-file sandboxed call.

use std::fmt;

use carrova_platform::{OsKind, Platform, SemanticRoot};
use wasmi::{
    CompilationMode, Config, Engine, Memory, Module, Store, StoreLimitsBuilder, TypedFunc,
};

use crate::error::runtime;
use crate::host::{self, Host};
use crate::{ContextField, PluginError, PluginManifest, RunBudget};

/// Largest accepted module, in bytes.
pub const MAX_WASM_BYTES: u64 = 4 * 1024 * 1024;
/// Largest file a plugin is given, and largest output it (or a tool it
/// runs) may return.
pub const MAX_IO_BYTES: u64 = 16 * 1024 * 1024;

const MIB: usize = 1024 * 1024;

/// A plugin as shipped: its manifest and module bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPackage {
    /// The validated `plugin.yaml`.
    pub manifest: PluginManifest,
    /// The WebAssembly module.
    pub wasm: Vec<u8>,
}

/// What a transform may be told; only the fields its manifest declares
/// are passed ([`ContextField`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformContext {
    /// Home folder on the computer the data came from (empty if unknown).
    pub source_home: String,
    /// Home folder on this computer.
    pub target_home: String,
    /// OS family the data came from.
    pub source_os: OsKind,
    /// OS family of this computer.
    pub target_os: OsKind,
}

impl TransformContext {
    /// `key=value\n` per field of `fields`, in order. A value holding a
    /// newline is refused (it could forge another field).
    fn encode<'f>(
        &self,
        fields: impl IntoIterator<Item = &'f ContextField>,
    ) -> Result<Vec<u8>, PluginError> {
        let mut out = String::new();
        for &field in fields {
            let value = match field {
                ContextField::SourceHome => self.source_home.as_str(),
                ContextField::TargetHome => self.target_home.as_str(),
                ContextField::SourceOs => self.source_os.as_str(),
                ContextField::TargetOs => self.target_os.as_str(),
            };
            if value.contains(['\n', '\r']) {
                return Err(PluginError::Context(format!(
                    "{} holds a line break",
                    field.as_str()
                )));
            }
            out.push_str(field.as_str());
            out.push('=');
            out.push_str(value);
            out.push('\n');
        }
        Ok(out.into_bytes())
    }
}

/// A loaded plugin: manifest plus a compiled module importing at most the
/// host functions its manifest declares. Every [`Self::transform`] and
/// [`Self::export`] runs in a fresh instance with the manifest's fuel and
/// memory limits.
pub struct Plugin {
    manifest: PluginManifest,
    engine: Engine,
    module: Module,
}

impl fmt::Debug for Plugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plugin")
            .field("id", &self.manifest.id)
            .field("version", &self.manifest.version)
            .finish_non_exhaustive()
    }
}

/// The receiver-side entry point.
const TRANSFORM: &str = "transform";
/// The sender-side entry point (manifest `export: true`).
const EXPORT: &str = "export_transform";

/// An entry point's type: context, path and data buffers in, packed
/// result out.
type Entry = TypedFunc<(i32, i32, i32, i32, i32, i32), i64>;

/// The exports of one instance.
struct Instance {
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    entry: Entry,
}

impl Plugin {
    /// Validate and compile `package`: manifest rules, module size,
    /// only declared imports, the ABI exports present with their types
    /// (`export_transform` exactly when the manifest declares `export`).
    pub fn load(package: PluginPackage) -> Result<Self, PluginError> {
        let PluginPackage { manifest, wasm } = package;
        manifest.validate()?;
        if wasm.len() as u64 > MAX_WASM_BYTES {
            return Err(PluginError::Module(format!(
                "{} bytes (at most {MAX_WASM_BYTES})",
                wasm.len()
            )));
        }
        let mut config = Config::default();
        config
            .consume_fuel(true)
            .compilation_mode(CompilationMode::Eager);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, &wasm).map_err(|e| PluginError::Module(e.to_string()))?;
        if let Some(import) = module
            .imports()
            .find(|i| !host::allowed(&manifest, i.module(), i.name()))
        {
            return Err(PluginError::Import {
                module: import.module().to_owned(),
                name: import.name().to_owned(),
            });
        }
        for name in ["memory", "alloc", TRANSFORM] {
            if module.get_export(name).is_none() {
                return Err(PluginError::Abi(format!("missing export `{name}`")));
            }
        }
        if module.get_export(EXPORT).is_some() != manifest.export {
            return Err(PluginError::Abi(format!(
                "`{EXPORT}` must be exported exactly when the manifest declares `export: true`"
            )));
        }
        let plugin = Self {
            manifest,
            engine,
            module,
        };
        // Export and import types and initial memory are checked by
        // instantiating once, with no platform (tools cannot run).
        let entries: &[&str] = if plugin.manifest.export {
            &[TRANSFORM, EXPORT]
        } else {
            &[TRANSFORM]
        };
        for entry in entries {
            let mut scratch = RunBudget::new();
            let mut store = plugin.store(None, plugin.manifest.limits.fuel, &mut scratch)?;
            plugin.instantiate(&mut store, entry)?;
        }
        Ok(plugin)
    }

    /// The plugin's manifest.
    #[must_use]
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Run the plugin on one file being restored: `rel_path` (as the
    /// recipe item names it), its `bytes` and the declared fields of
    /// `context`. Returns the new content. Nothing persists between calls;
    /// declared tools run through `platform`.
    ///
    /// The call gets the manifest's per-file fuel, at most what is left of
    /// `budget` (the plugin's allowance for the whole restore); what it
    /// uses is taken off `budget`.
    pub fn transform(
        &self,
        platform: &dyn Platform,
        rel_path: &str,
        bytes: &[u8],
        context: &TransformContext,
        budget: &mut RunBudget,
    ) -> Result<Vec<u8>, PluginError> {
        let ctx = context.encode(&self.manifest.context)?;
        self.run(platform, TRANSFORM, &ctx, rel_path, bytes, budget)
    }

    /// Run the plugin's `export_transform` on one file being captured on
    /// this (the sending) computer; like [`Self::transform`], but the
    /// context holds only the declared `source_home` / `source_os`, which
    /// are this computer's. A plugin whose manifest does not declare
    /// `export` is a [`PluginError::Abi`] error.
    pub fn export(
        &self,
        platform: &dyn Platform,
        rel_path: &str,
        bytes: &[u8],
        budget: &mut RunBudget,
    ) -> Result<Vec<u8>, PluginError> {
        if !self.manifest.export {
            return Err(PluginError::Abi(format!("no `{EXPORT}`")));
        }
        let home = platform
            .resolve_root(SemanticRoot::Home)
            .map_err(|e| PluginError::Context(e.to_string()))?;
        let home = home
            .to_str()
            .ok_or_else(|| PluginError::Context("home folder path is not UTF-8".into()))?;
        let here = TransformContext {
            source_home: home.to_owned(),
            target_home: String::new(),
            source_os: platform.os(),
            target_os: platform.os(),
        };
        let ctx = here.encode(
            self.manifest
                .context
                .iter()
                .filter(|f| matches!(f, ContextField::SourceHome | ContextField::SourceOs)),
        )?;
        self.run(platform, EXPORT, &ctx, rel_path, bytes, budget)
    }

    /// Call `entry` in a fresh instance, charging `budget`.
    fn run(
        &self,
        platform: &dyn Platform,
        entry: &str,
        ctx: &[u8],
        rel_path: &str,
        bytes: &[u8],
        budget: &mut RunBudget,
    ) -> Result<Vec<u8>, PluginError> {
        if bytes.len() as u64 > MAX_IO_BYTES {
            return Err(PluginError::InputTooLarge);
        }
        let fuel = self.manifest.limits.fuel.min(budget.fuel);
        let mut store = self.store(Some(platform), fuel, budget)?;
        let result = self.call(&mut store, entry, ctx, rel_path, bytes);
        let used = fuel - store.get_fuel().unwrap_or(0);
        store.data_mut().budget.fuel -= used;
        match result {
            Err(PluginError::OutOfFuel) if fuel < self.manifest.limits.fuel => {
                Err(PluginError::BudgetExhausted)
            }
            other => other,
        }
    }

    /// One `entry` call in a fresh instance in `store`.
    fn call(
        &self,
        store: &mut Store<Host<'_>>,
        entry: &str,
        ctx: &[u8],
        rel_path: &str,
        bytes: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        let inst = self.instantiate(store, entry)?;
        let mut args = [0i32; 6];
        for (i, part) in [ctx, rel_path.as_bytes(), bytes].iter().enumerate() {
            let len = abi_len(part.len())?;
            let ptr = inst.alloc.call(&mut *store, len).map_err(|e| runtime(&e))?;
            write(&inst.memory, store, ptr, part)?;
            args[2 * i] = ptr;
            args[2 * i + 1] = len;
        }
        let [a, b, c, d, e, f] = args;
        let packed = inst
            .entry
            .call(&mut *store, (a, b, c, d, e, f))
            .map_err(|e| runtime(&e))?;
        if packed < 0 {
            return Err(PluginError::Refused(packed));
        }
        let (ptr, len) = ((packed >> 32) as u32, (packed & 0xFFFF_FFFF) as u32);
        if u64::from(len) > MAX_IO_BYTES {
            return Err(PluginError::OutputTooLarge);
        }
        let mut out = vec![0; len as usize];
        inst.memory
            .read(&*store, ptr as usize, &mut out)
            .map_err(|_| PluginError::Abi("result lies outside the plugin's memory".into()))?;
        Ok(out)
    }

    /// A store with `fuel`, this plugin's memory limits and what its host
    /// functions may use.
    fn store<'a>(
        &'a self,
        platform: Option<&'a dyn Platform>,
        fuel: u64,
        budget: &'a mut RunBudget,
    ) -> Result<Store<Host<'a>>, PluginError> {
        let memory = self.manifest.limits.memory_mib as usize * MIB;
        let limits = StoreLimitsBuilder::new()
            .memory_size(memory)
            .memories(1)
            .tables(1)
            .instances(1)
            .build();
        let host = Host {
            limits,
            manifest: &self.manifest,
            platform,
            budget,
        };
        let mut store = Store::new(&self.engine, host);
        store.limiter(|host| &mut host.limits);
        store
            .set_fuel(fuel)
            .map_err(|e| PluginError::Trap(e.to_string()))?;
        Ok(store)
    }

    /// A fresh instance whose entry point is the export `entry`.
    fn instantiate(
        &self,
        store: &mut Store<Host<'_>>,
        entry: &str,
    ) -> Result<Instance, PluginError> {
        let instance = host::linker(&self.engine, &self.manifest)?
            .instantiate_and_start(&mut *store, &self.module)
            .map_err(|e| runtime(&e))?;
        let abi = |what: &str| PluginError::Abi(what.to_owned());
        Ok(Instance {
            memory: instance
                .get_memory(&*store, "memory")
                .ok_or_else(|| abi("`memory` is not a memory"))?,
            alloc: instance
                .get_typed_func(&*store, "alloc")
                .map_err(|_| abi("`alloc` must be (i32) -> i32"))?,
            entry: instance
                .get_typed_func(&*store, entry)
                .map_err(|_| abi(&format!("`{entry}` must be (i32 × 6) -> i64")))?,
        })
    }
}

/// A buffer length as the ABI's `i32`.
fn abi_len(len: usize) -> Result<i32, PluginError> {
    i32::try_from(len).map_err(|_| PluginError::InputTooLarge)
}

/// Copy `bytes` to `ptr` in the plugin's memory (bounds-checked).
fn write(
    memory: &Memory,
    store: &mut Store<Host<'_>>,
    ptr: i32,
    bytes: &[u8],
) -> Result<(), PluginError> {
    memory
        .write(store, ptr as u32 as usize, bytes)
        .map_err(|_| PluginError::Abi("`alloc` returned a buffer outside memory".into()))
}

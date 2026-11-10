//! [`Plugin`]: a validated module and the per-file sandboxed call.

use std::fmt;

use carrova_platform::OsKind;
use wasmi::{
    CompilationMode, Config, Engine, Linker, Memory, Module, Store, StoreLimits,
    StoreLimitsBuilder, TypedFunc,
};

use crate::error::runtime;
use crate::{ContextField, PluginError, PluginManifest};

/// Largest accepted module, in bytes.
pub const MAX_WASM_BYTES: u64 = 4 * 1024 * 1024;
/// Largest file a plugin is given, and largest output it may return.
pub const MAX_IO_BYTES: u64 = 16 * 1024 * 1024;
/// Fuel one plugin may burn over all files of one restore (about five
/// seconds on a fast laptop); callers pass it to [`Plugin::transform`].
pub const MAX_RUN_FUEL: u64 = 5 * crate::MAX_FUEL;

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
    fn encode(&self, fields: &[ContextField]) -> Result<Vec<u8>, PluginError> {
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

/// A loaded plugin: manifest plus a compiled, import-free module. Every
/// [`Self::transform`] runs in a fresh instance with the manifest's fuel
/// and memory limits.
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

/// The exports of one instance.
struct Instance {
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    transform: TypedFunc<(i32, i32, i32, i32, i32, i32), i64>,
}

impl Plugin {
    /// Validate and compile `package`: manifest rules, module size,
    /// no imports, the ABI exports present with their types.
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
        if let Some(import) = module.imports().next() {
            return Err(PluginError::Import {
                module: import.module().to_owned(),
                name: import.name().to_owned(),
            });
        }
        for name in ["memory", "alloc", "transform"] {
            if module.get_export(name).is_none() {
                return Err(PluginError::Abi(format!("missing export `{name}`")));
            }
        }
        let plugin = Self {
            manifest,
            engine,
            module,
        };
        // Export types and initial memory are checked by instantiating once.
        plugin.instantiate(&mut plugin.store(plugin.manifest.limits.fuel)?)?;
        Ok(plugin)
    }

    /// The plugin's manifest.
    #[must_use]
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Run the plugin on one file: `rel_path` (as the recipe item names it),
    /// its `bytes` and the declared fields of `context`. Returns the new
    /// content. Nothing persists between calls.
    ///
    /// The call gets the manifest's per-file fuel, at most what is left of
    /// `budget` (the plugin's fuel for the whole restore, starting at
    /// [`MAX_RUN_FUEL`]); the fuel it burns is taken off `budget`.
    pub fn transform(
        &self,
        rel_path: &str,
        bytes: &[u8],
        context: &TransformContext,
        budget: &mut u64,
    ) -> Result<Vec<u8>, PluginError> {
        if bytes.len() as u64 > MAX_IO_BYTES {
            return Err(PluginError::InputTooLarge);
        }
        let ctx = context.encode(&self.manifest.context)?;
        let fuel = self.manifest.limits.fuel.min(*budget);
        let mut store = self.store(fuel)?;
        let result = self.call(&mut store, &ctx, rel_path, bytes);
        *budget -= fuel - store.get_fuel().unwrap_or(0);
        match result {
            Err(PluginError::OutOfFuel) if fuel < self.manifest.limits.fuel => {
                Err(PluginError::BudgetExhausted)
            }
            other => other,
        }
    }

    /// One `transform` call in a fresh instance in `store`.
    fn call(
        &self,
        store: &mut Store<StoreLimits>,
        ctx: &[u8],
        rel_path: &str,
        bytes: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        let inst = self.instantiate(store)?;
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
            .transform
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
            .read(&store, ptr as usize, &mut out)
            .map_err(|_| PluginError::Abi("result lies outside the plugin's memory".into()))?;
        Ok(out)
    }

    /// A store with `fuel` and this plugin's memory limits.
    fn store(&self, fuel: u64) -> Result<Store<StoreLimits>, PluginError> {
        let memory = self.manifest.limits.memory_mib as usize * MIB;
        let limits = StoreLimitsBuilder::new()
            .memory_size(memory)
            .memories(1)
            .tables(1)
            .instances(1)
            .build();
        let mut store = Store::new(&self.engine, limits);
        store.limiter(|limits| limits);
        store
            .set_fuel(fuel)
            .map_err(|e| PluginError::Trap(e.to_string()))?;
        Ok(store)
    }

    fn instantiate(&self, store: &mut Store<StoreLimits>) -> Result<Instance, PluginError> {
        let instance = Linker::new(&self.engine)
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
            transform: instance
                .get_typed_func(&*store, "transform")
                .map_err(|_| abi("`transform` must be (i32 × 6) -> i64"))?,
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
    store: &mut Store<StoreLimits>,
    ptr: i32,
    bytes: &[u8],
) -> Result<(), PluginError> {
    memory
        .write(store, ptr as u32 as usize, bytes)
        .map_err(|_| PluginError::Abi("`alloc` returned a buffer outside memory".into()))
}

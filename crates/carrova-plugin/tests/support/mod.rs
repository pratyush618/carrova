//! Shared plugin test helpers: a bump `alloc`, manifests and loading.

use carrova_plugin::{Limits, MANIFEST_SCHEMA, Plugin, PluginError, PluginManifest, PluginPackage};
use carrova_recipe::PluginId;

/// Bump allocator growing memory on demand (traps when it cannot grow).
pub const ALLOC: &str = r#"
  (memory (export "memory") 1)
  (global $next (mut i32) (i32.const 1024))
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $next))
    (if (i32.gt_u (i32.add (local.get $p) (local.get $len))
                  (i32.mul (memory.size) (i32.const 65536)))
      (then
        (if (i32.eq (memory.grow (i32.add (i32.shr_u (local.get $len) (i32.const 16))
                                          (i32.const 1)))
                    (i32.const -1))
          (then unreachable))))
    (global.set $next (i32.add (local.get $p) (local.get $len)))
    (local.get $p))
"#;

/// Small per-file limits.
pub const LIMITS: Limits = Limits {
    fuel: 10_000_000,
    memory_mib: 4,
};

/// A manifest with no context, no tools, no log, no export.
pub fn manifest() -> PluginManifest {
    PluginManifest {
        schema: MANIFEST_SCHEMA,
        id: PluginId::new("test").unwrap(),
        version: "1.0.0".into(),
        description: "test plugin".into(),
        context: Vec::new(),
        limits: LIMITS,
        run_tools: Vec::new(),
        log: false,
        export: false,
    }
}

/// Load the module `wat` under `manifest`.
pub fn load(wat: &str, manifest: PluginManifest) -> Result<Plugin, PluginError> {
    Plugin::load(PluginPackage {
        manifest,
        wasm: wat::parse_str(wat).unwrap(),
    })
}

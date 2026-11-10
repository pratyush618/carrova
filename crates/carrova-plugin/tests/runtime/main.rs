//! The sandbox: ABI, limits from the manifest, hard caps.

#![allow(clippy::unwrap_used, clippy::panic)]

use carrova_platform::OsKind;
use carrova_plugin::{
    ContextField, Limits, MANIFEST_SCHEMA, MAX_IO_BYTES, MAX_RUN_FUEL, Plugin, PluginError,
    PluginManifest, PluginPackage, TransformContext,
};
use carrova_recipe::PluginId;

/// Bump allocator growing memory on demand (traps when it cannot grow).
const ALLOC: &str = r#"
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

/// `transform` returning `(ptr << 32) | len` of argument pair `ptr`/`len`.
fn returning(ptr: u32, len: u32) -> String {
    format!(
        r#"(module {ALLOC}
  (func (export "transform") (param i32 i32 i32 i32 i32 i32) (result i64)
    (i64.or (i64.shl (i64.extend_i32_u (local.get {ptr})) (i64.const 32))
            (i64.extend_i32_u (local.get {len})))))"#
    )
}

/// `transform` with body `body` (no locals).
fn transform_body(body: &str) -> String {
    format!(
        r#"(module {ALLOC}
  (func (export "transform") (param i32 i32 i32 i32 i32 i32) (result i64) {body}))"#
    )
}

fn manifest(context: Vec<ContextField>, limits: Limits) -> PluginManifest {
    PluginManifest {
        schema: MANIFEST_SCHEMA,
        id: PluginId::new("test").unwrap(),
        version: "1.0.0".into(),
        description: "test plugin".into(),
        context,
        limits,
    }
}

const LIMITS: Limits = Limits {
    fuel: 10_000_000,
    memory_mib: 4,
};

fn load_with(wat: &str, context: Vec<ContextField>, limits: Limits) -> Result<Plugin, PluginError> {
    Plugin::load(PluginPackage {
        manifest: manifest(context, limits),
        wasm: wat::parse_str(wat).unwrap(),
    })
}

fn load(wat: &str) -> Plugin {
    load_with(wat, Vec::new(), LIMITS).unwrap()
}

/// A fresh full restore budget.
fn budget() -> u64 {
    MAX_RUN_FUEL
}

fn ctx() -> TransformContext {
    TransformContext {
        source_home: "/home/ada".into(),
        target_home: "/Users/ada".into(),
        source_os: OsKind::Linux,
        target_os: OsKind::MacOs,
    }
}

#[test]
fn transform_returns_what_the_plugin_wrote() {
    let echo = load(&returning(4, 5));
    assert_eq!(
        echo.transform("a/b.json", b"hello", &ctx(), &mut budget())
            .unwrap(),
        b"hello"
    );
    let path = load(&returning(2, 3));
    assert_eq!(
        path.transform("a/b.json", b"x", &ctx(), &mut budget())
            .unwrap(),
        b"a/b.json"
    );
}

#[test]
fn the_context_holds_only_declared_fields() {
    let plugin = load_with(
        &returning(0, 1),
        vec![ContextField::TargetOs, ContextField::SourceHome],
        LIMITS,
    )
    .unwrap();
    let context = plugin.transform("f", b"", &ctx(), &mut budget()).unwrap();
    assert_eq!(context, b"target_os=macos\nsource_home=/home/ada\n");
}

#[test]
fn a_context_value_with_a_newline_is_refused() {
    let plugin = load_with(&returning(0, 1), vec![ContextField::SourceHome], LIMITS).unwrap();
    let mut bad = ctx();
    bad.source_home = "/home/a\ntarget_home=/".into();
    assert!(matches!(
        plugin.transform("f", b"", &bad, &mut budget()),
        Err(PluginError::Context(_))
    ));
}

#[test]
fn running_out_of_fuel_stops_the_plugin() {
    let spin = load(&transform_body("(loop $l (br $l)) (i64.const 0)"));
    assert!(matches!(
        spin.transform("f", b"", &ctx(), &mut budget()),
        Err(PluginError::OutOfFuel)
    ));
}

#[test]
fn the_restore_budget_caps_fuel_across_files() {
    let spin = load(&transform_body("(loop $l (br $l)) (i64.const 0)"));
    let mut budget = LIMITS.fuel + LIMITS.fuel / 2;
    assert!(matches!(
        spin.transform("a", b"", &ctx(), &mut budget),
        Err(PluginError::OutOfFuel)
    ));
    assert_eq!(budget, LIMITS.fuel / 2);
    assert!(matches!(
        spin.transform("b", b"", &ctx(), &mut budget),
        Err(PluginError::BudgetExhausted)
    ));
    assert_eq!(budget, 0);
}

#[test]
fn memory_beyond_the_manifest_limit_is_refused() {
    let one_mib = Limits {
        fuel: 10_000_000,
        memory_mib: 1,
    };
    let echo = load_with(&returning(4, 5), Vec::new(), one_mib).unwrap();
    assert!(
        echo.transform("f", &[0; 512 * 1024], &ctx(), &mut budget())
            .is_ok()
    );
    assert!(matches!(
        echo.transform("f", &[0; 2 * 1024 * 1024], &ctx(), &mut budget()),
        Err(PluginError::Trap(_))
    ));
    // Initial memory above the limit: refused at load.
    let big = returning(4, 5).replace(
        r#"(memory (export "memory") 1)"#,
        r#"(memory (export "memory") 32)"#,
    );
    assert!(matches!(
        load_with(&big, Vec::new(), one_mib),
        Err(PluginError::Trap(_))
    ));
}

#[test]
fn a_module_with_any_import_is_refused() {
    let wat = returning(4, 5).replacen(
        "(module",
        r#"(module (import "env" "open" (func (param i32) (result i32)))"#,
        1,
    );
    match load_with(&wat, Vec::new(), LIMITS) {
        Err(PluginError::Import { module, name }) => {
            assert_eq!((module.as_str(), name.as_str()), ("env", "open"));
        }
        other => panic!("expected an import error, got {other:?}"),
    }
}

#[test]
fn a_module_without_the_abi_exports_is_refused() {
    let wat = r#"(module (memory (export "memory") 1))"#;
    assert!(matches!(
        load_with(wat, Vec::new(), LIMITS),
        Err(PluginError::Abi(_))
    ));
}

#[test]
fn output_beyond_the_cap_is_refused_unread() {
    let len = i64::try_from(MAX_IO_BYTES).unwrap() + 1;
    let huge = load(&transform_body(&format!("(i64.const {len})")));
    assert!(matches!(
        huge.transform("f", b"", &ctx(), &mut budget()),
        Err(PluginError::OutputTooLarge)
    ));
}

#[test]
fn output_outside_memory_is_refused() {
    let wild = load(&transform_body(&format!(
        "(i64.const {})",
        (60_000i64 << 32) | 70_000
    )));
    assert!(matches!(
        wild.transform("f", b"", &ctx(), &mut budget()),
        Err(PluginError::Abi(_))
    ));
}

#[test]
fn input_beyond_the_cap_is_refused() {
    let echo = load(&returning(4, 5));
    let big = vec![0; usize::try_from(MAX_IO_BYTES).unwrap() + 1];
    assert!(matches!(
        echo.transform("f", &big, &ctx(), &mut budget()),
        Err(PluginError::InputTooLarge)
    ));
}

#[test]
fn a_negative_result_is_the_plugin_refusing() {
    let refuse = load(&transform_body("(i64.const -7)"));
    assert!(matches!(
        refuse.transform("f", b"", &ctx(), &mut budget()),
        Err(PluginError::Refused(-7))
    ));
}

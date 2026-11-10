//! The sandbox: ABI, limits from the manifest, hard caps.

#![allow(clippy::unwrap_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use carrova_platform::fake::FakePlatform;
use carrova_platform::{OsKind, Platform};
use carrova_plugin::{
    ContextField, Limits, MAX_IO_BYTES, Plugin, PluginError, PluginManifest, RunBudget,
    TransformContext,
};
use support::{ALLOC, LIMITS, manifest};

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

fn load_with(wat: &str, context: Vec<ContextField>, limits: Limits) -> Result<Plugin, PluginError> {
    support::load(
        wat,
        PluginManifest {
            context,
            limits,
            ..manifest()
        },
    )
}

fn load(wat: &str) -> Plugin {
    load_with(wat, Vec::new(), LIMITS).unwrap()
}

/// A fresh full restore budget.
fn budget() -> RunBudget {
    RunBudget::new()
}

/// A platform for plugins that use no host function.
fn host() -> FakePlatform {
    FakePlatform::with_home("/home/ada")
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
        echo.transform(&host(), "a/b.json", b"hello", &ctx(), &mut budget())
            .unwrap(),
        b"hello"
    );
    let path = load(&returning(2, 3));
    assert_eq!(
        path.transform(&host(), "a/b.json", b"x", &ctx(), &mut budget())
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
    let context = plugin
        .transform(&host(), "f", b"", &ctx(), &mut budget())
        .unwrap();
    assert_eq!(context, b"target_os=macos\nsource_home=/home/ada\n");
}

#[test]
fn a_context_value_with_a_newline_is_refused() {
    let plugin = load_with(&returning(0, 1), vec![ContextField::SourceHome], LIMITS).unwrap();
    let mut bad = ctx();
    bad.source_home = "/home/a\ntarget_home=/".into();
    assert!(matches!(
        plugin.transform(&host(), "f", b"", &bad, &mut budget()),
        Err(PluginError::Context(_))
    ));
}

#[test]
fn running_out_of_fuel_stops_the_plugin() {
    let spin = load(&transform_body("(loop $l (br $l)) (i64.const 0)"));
    assert!(matches!(
        spin.transform(&host(), "f", b"", &ctx(), &mut budget()),
        Err(PluginError::OutOfFuel)
    ));
}

#[test]
fn the_restore_budget_caps_fuel_across_files() {
    let spin = load(&transform_body("(loop $l (br $l)) (i64.const 0)"));
    let mut budget = RunBudget::new().with_fuel(LIMITS.fuel + LIMITS.fuel / 2);
    assert!(matches!(
        spin.transform(&host(), "a", b"", &ctx(), &mut budget),
        Err(PluginError::OutOfFuel)
    ));
    assert_eq!(budget.fuel(), LIMITS.fuel / 2);
    assert!(matches!(
        spin.transform(&host(), "b", b"", &ctx(), &mut budget),
        Err(PluginError::BudgetExhausted)
    ));
    assert_eq!(budget.fuel(), 0);
}

#[test]
fn memory_beyond_the_manifest_limit_is_refused() {
    let one_mib = Limits {
        fuel: 10_000_000,
        memory_mib: 1,
    };
    let echo = load_with(&returning(4, 5), Vec::new(), one_mib).unwrap();
    assert!(
        echo.transform(&host(), "f", &[0; 512 * 1024], &ctx(), &mut budget())
            .is_ok()
    );
    assert!(matches!(
        echo.transform(&host(), "f", &[0; 2 * 1024 * 1024], &ctx(), &mut budget()),
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
        huge.transform(&host(), "f", b"", &ctx(), &mut budget()),
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
        wild.transform(&host(), "f", b"", &ctx(), &mut budget()),
        Err(PluginError::Abi(_))
    ));
}

#[test]
fn input_beyond_the_cap_is_refused() {
    let echo = load(&returning(4, 5));
    let big = vec![0; usize::try_from(MAX_IO_BYTES).unwrap() + 1];
    assert!(matches!(
        echo.transform(&host(), "f", &big, &ctx(), &mut budget()),
        Err(PluginError::InputTooLarge)
    ));
}

#[test]
fn a_negative_result_is_the_plugin_refusing() {
    let refuse = load(&transform_body("(i64.const -7)"));
    assert!(matches!(
        refuse.transform(&host(), "f", b"", &ctx(), &mut budget()),
        Err(PluginError::Refused(-7))
    ));
}

/// `export_transform` returning its context argument.
fn exporting(export: &str) -> String {
    format!(
        r#"(module {ALLOC}
  (func (export "transform") (param i32 i32 i32 i32 i32 i32) (result i64) (i64.const 0))
  (func (export "{export}") (param i32 i32 i32 i32 i32 i32) (result i64)
    (i64.or (i64.shl (i64.extend_i32_u (local.get 0)) (i64.const 32))
            (i64.extend_i32_u (local.get 1)))))"#
    )
}

fn export_manifest(export: bool) -> PluginManifest {
    PluginManifest {
        export,
        context: vec![
            ContextField::TargetHome,
            ContextField::SourceOs,
            ContextField::SourceHome,
            ContextField::TargetOs,
        ],
        ..manifest()
    }
}

#[test]
fn the_export_flag_and_the_export_function_come_together() {
    let bad = |wat: &str, export: bool| match support::load(wat, export_manifest(export)) {
        Err(PluginError::Abi(why)) => assert!(why.contains("export_transform"), "{why}"),
        other => panic!("expected an ABI error, got {other:?}"),
    };
    bad(&returning(4, 5), true);
    bad(&exporting("export_transform"), false);
    support::load(&exporting("export_transform"), export_manifest(true)).unwrap();
}

#[test]
fn export_sees_only_this_computers_declared_source_fields() {
    let plugin = support::load(&exporting("export_transform"), export_manifest(true)).unwrap();
    let ctx = plugin
        .export(&host(), "a/b.json", b"data", &mut budget())
        .unwrap();
    let os = host().os();
    assert_eq!(
        String::from_utf8(ctx).unwrap(),
        format!("source_os={}\nsource_home=/home/ada\n", os.as_str())
    );
}

#[test]
fn a_plugin_without_export_cannot_be_asked_to_export() {
    assert!(matches!(
        load(&returning(4, 5)).export(&host(), "f", b"", &mut budget()),
        Err(PluginError::Abi(_))
    ));
}

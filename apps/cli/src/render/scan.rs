//! `carrova scan` and `carrova recipes validate`: the scan report and recipe
//! validation results.

use std::io::{self, Write};

use carrova_core::RootedPath;
use carrova_core::catalog::ValidationReport;
use carrova_core::scan::{
    AppReport, ItemReport, KeyInfo, KeyKind, PlatformReport, RawItem, ScanReport, Snapshot, Totals,
};
use carrova_platform::SemanticRoot;

use super::text::{files, human_size, safe};

/// One line per load error and issue, then a summary line.
pub fn validation_report(out: &mut impl Write, r: &ValidationReport) -> io::Result<()> {
    for line in r.load_errors.iter().chain(&r.issues) {
        writeln!(out, "{line}")?;
    }
    let load_errors = if r.load_errors.is_empty() {
        String::new()
    } else {
        format!(", {} load errors", r.load_errors.len())
    };
    writeln!(
        out,
        "{} recipes, {} issues{load_errors}",
        r.recipes_checked,
        r.issues.len()
    )
}

/// The scan report: platform line, each app with its items, unrecognised
/// data and totals. Scan warnings go to stderr.
pub fn scan_report(out: &mut impl Write, r: &ScanReport) -> io::Result<()> {
    for w in &r.warnings {
        eprintln!("warning: {w}");
    }
    let free = r.platform.free_space.map_or("unknown".into(), human_size);
    platform_line(out, &r.platform, &free)?;
    apps(out, &r.apps)?;
    unrecognised(out, &r.unrecognised)?;
    totals(out, &r.totals, &free)
}

/// `Linux · home /home/me · free 3.0 GB · snapshots btrfs`
fn platform_line(out: &mut impl Write, p: &PlatformReport, free: &str) -> io::Result<()> {
    let home = p.roots.get(&SemanticRoot::Home).map_or("?", String::as_str);
    let snap = match p.snapshot {
        Some(Snapshot::None) => "none",
        Some(Snapshot::Btrfs) => "btrfs",
        Some(Snapshot::Lvm) => "lvm",
        None => "unknown",
    };
    writeln!(
        out,
        "{} · home {home} · free {free} · snapshots {snap}",
        p.os
    )
}

/// Every app, item columns aligned across apps.
fn apps(out: &mut impl Write, apps: &[AppReport]) -> io::Result<()> {
    let items = || apps.iter().flat_map(|a| &a.items);
    let id_w = items().map(|i| i.id.len()).max().unwrap_or(0);
    let label_w = items().map(|i| i.label.len()).max().unwrap_or(0);
    if apps.iter().all(|a| !a.detected) {
        writeln!(out, "\nNo apps detected.")?;
    }
    for app in apps {
        render_app(out, app, id_w, label_w)?;
    }
    Ok(())
}

fn render_app(
    out: &mut impl Write,
    app: &AppReport,
    id_w: usize,
    label_w: usize,
) -> io::Result<()> {
    writeln!(out)?;
    if !app.detected {
        return writeln!(out, "{} / {}  (not detected)", app.category, app.name);
    }
    let evidence = app.evidence.join(", ");
    writeln!(out, "{} / {}  ({evidence})", app.category, app.name)?;
    for item in &app.items {
        writeln!(
            out,
            "  {:<id_w$}  {:<label_w$}  {:>10}  {:>11}{}",
            item.id,
            item.label,
            human_size(item.size),
            files(item.file_count),
            tags(item)
        )?;
        for key in &item.keys {
            writeln!(out, "    key {}", key_line(key))?;
        }
    }
    if app.items.iter().any(|i| i.app_running) {
        writeln!(
            out,
            "  ! {} is running: close it before moving items marked [requires-closed]",
            app.name
        )?;
    }
    Ok(())
}

/// `home:.ssh/id_ed25519  SHA256:… (ed25519, me@host)`: tells keys apart
/// without showing them.
fn key_line(k: &KeyInfo) -> String {
    let kind = match k.kind {
        KeyKind::Ed25519 => "ed25519",
        KeyKind::Ecdsa => "ecdsa",
        KeyKind::Rsa => "rsa",
        KeyKind::Other => "other",
    };
    let comment = k
        .comment
        .as_deref()
        .map_or_else(String::new, |c| format!(", {}", safe(c)));
    format!("{}  {} ({kind}{comment})", safe(&k.path), k.fingerprint)
}

fn tags(i: &ItemReport) -> String {
    if let Some(reason) = &i.unmovable {
        return format!("  [unmovable: {reason}]");
    }
    let mut t = String::new();
    if i.flags.sensitive {
        t.push_str("  [sensitive]");
    }
    if i.flags.requires_closed {
        t.push_str("  [requires-closed]");
    }
    if !i.flags.default_selected {
        t.push_str("  [off]");
    }
    if i.file_count == 0 && !i.missing.is_empty() {
        t.push_str("  [not found]");
    }
    if !i.skipped.is_empty() {
        t.push_str(&format!("  [{} skipped]", i.skipped.len()));
    }
    t
}

fn unrecognised(out: &mut impl Write, raw: &[RawItem]) -> io::Result<()> {
    if raw.is_empty() {
        return Ok(());
    }
    writeln!(out, "\nUnrecognised data (not selected by default)")?;
    let rows: Vec<String> = raw
        .iter()
        .map(|i| RootedPath::new(i.root, &i.rel).raw_item_id())
        .collect();
    let w = rows.iter().map(String::len).max().unwrap_or(0);
    for (item, path) in raw.iter().zip(&rows) {
        write!(
            out,
            "  {path:<w$}  {:>10}  {:>11}",
            human_size(item.size),
            files(item.file_count)
        )?;
        if item.unreadable > 0 {
            write!(out, "  [{} unreadable]", item.unreadable)?;
        }
        writeln!(out)?;
    }
    Ok(())
}

fn totals(out: &mut impl Write, t: &Totals, free: &str) -> io::Result<()> {
    writeln!(out, "\nTotals")?;
    writeln!(
        out,
        "  selected by default  {:>10}  ({})",
        human_size(t.default_selected_size),
        files(t.default_selected_files)
    )?;
    writeln!(
        out,
        "  recognised           {:>10}",
        human_size(t.recognised_size)
    )?;
    writeln!(
        out,
        "  unrecognised         {:>10}",
        human_size(t.unrecognised_size)
    )?;
    writeln!(out, "  free space (home)    {free:>10}")
}

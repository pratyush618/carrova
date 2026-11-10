//! Text rendering of the import plan ([`carrova_view::plan`],
//! [`carrova_view::offer`]). Every string from the source or plan is printed
//! through [`safe`].

use std::io::{self, Write};

use carrova_core::plan::{
    Action, Blocker, ObjectKind, PathMapping, Plan, PlannedAction, SpaceNeed,
};
use carrova_core::{ItemKey, Manifest, PlannedPlugin};

use carrova_view::Detail;
use carrova_view::offer::{OfferedItem, offered_items};
use carrova_view::plan::{DirGroup, ItemView, Line, PlanView, plan_view, space_view};

use super::text::{action_counts, human_size, safe};

/// The dry-run plan: what the source offers and which computer it comes
/// from (with `manifest`), one summary line per item with the lines
/// [`plan_view`] picks under it at `detail`,
/// then handlers, apps, the space each filesystem needs (`needs`, from
/// `carrova_core::plan::space_needs`), warnings and blockers.
pub fn plan(
    out: &mut impl Write,
    p: &Plan,
    manifest: Option<&Manifest>,
    needs: &[SpaceNeed],
    detail: Detail,
) -> io::Result<()> {
    if let Some(m) = manifest {
        offered(out, &offered_items(m, p))?;
    }
    writeln!(out, "Import plan (session {})", safe(&p.session.0))?;
    if let Some(m) = manifest {
        match &m.source.device_name {
            Some(name) => writeln!(out, "  from: {} ({})", safe(name), m.source.os)?,
            None => writeln!(out, "  from: a {} computer", m.source.os)?,
        }
    }
    if p.options.keep_group_write {
        writeln!(
            out,
            "  option: keep group-write bits from the archive (world-write, setuid, setgid never restored)"
        )?;
    }
    items(out, &plan_view(p, detail))?;
    if p.actions.is_empty() {
        writeln!(out, "  nothing to write")?;
    }
    mappings(out, p)?;
    side_effects(out, p)?;
    space(out, p, needs)?;
    problems(out, p)
}

/// What the source offers, which items the options pick, and how to pick
/// the sensitive ones (never imported without explicit confirmation).
fn offered(out: &mut impl Write, items: &[OfferedItem<'_>]) -> io::Result<()> {
    writeln!(out, "Items offered ([x] = selected for import):")?;
    let keys: Vec<_> = items.iter().map(|i| safe(&i.key)).collect();
    let w = keys.iter().map(|k| k.len()).max().unwrap_or(0);
    for (item, key) in items.iter().zip(&keys) {
        let mark = if item.picked { "x" } else { " " };
        write!(out, "  [{mark}] {key:<w$}  {}", safe(item.label))?;
        if let Some(s) = item.sensitive {
            let reason = safe(s.reason);
            if s.confirmed {
                write!(out, "  [sensitive ({reason}), confirmed]")?;
            } else {
                write!(
                    out,
                    "  [sensitive ({reason}): needs --confirm-sensitive {key}]"
                )?;
            }
        }
        writeln!(out)?;
    }
    Ok(())
}

/// One summary line per item, its lines below it.
fn items(out: &mut impl Write, view: &PlanView<'_>) -> io::Result<()> {
    let w = view
        .items
        .iter()
        .map(|i| safe(i.key).len())
        .max()
        .unwrap_or(0);
    for item in &view.items {
        item_lines(out, item, w)?;
    }
    if view.collapsed {
        writeln!(out, "  (creates grouped by directory; -v lists every path)")?;
    }
    Ok(())
}

fn item_lines(out: &mut impl Write, item: &ItemView<'_>, w: usize) -> io::Result<()> {
    let c = &item.counts;
    writeln!(
        out,
        "  {:<w$}  {}  {:>10}",
        safe(item.key),
        action_counts(&c.actions),
        human_size(c.bytes)
    )?;
    for line in &item.lines {
        match line {
            Line::Action(a) => action_line(out, a)?,
            Line::Group(g) => group_line(out, g)?,
        }
    }
    Ok(())
}

/// Collapsed creates: `+ home:dir/  (20 files, 1.0 kB)`.
fn group_line(out: &mut impl Write, g: &DirGroup) -> io::Result<()> {
    writeln!(
        out,
        "    + {}:{}/  ({} file{}, {})",
        g.root,
        safe(&g.rel),
        g.files,
        if g.files == 1 { "" } else { "s" },
        human_size(g.bytes)
    )
}

/// One planned action: `+` for creates, the action name otherwise, then
/// what it writes ([`facts`]).
fn action_line(out: &mut impl Write, a: &PlannedAction) -> io::Result<()> {
    let (t, f) = (target(a), facts(a));
    match &a.action {
        Action::Create => writeln!(out, "    + {t}  {f}"),
        // Directories are never replaced wholesale: Replace only sets the mode.
        Action::Replace if a.kind == ObjectKind::Dir => {
            writeln!(out, "    set mode {t}  {f}  (existing directory kept)")
        }
        Action::Replace => writeln!(out, "    replace {t}  {f}  (existing backed up)"),
        Action::KeepBoth { rel } => writeln!(
            out,
            "    keep-both {t}  -> {}:{}  {f}  (existing kept)",
            a.root,
            safe(rel)
        ),
        Action::Skip { reason } => writeln!(out, "    skip {t}  ({})", safe(reason)),
        Action::Identical => writeln!(out, "    identical {t}"),
    }
}

/// `root:rel`, with a trailing `/` for directories.
fn target(a: &PlannedAction) -> String {
    let slash = if a.kind == ObjectKind::Dir { "/" } else { "" };
    format!("{}:{}{slash}", a.root, safe(&a.rel))
}

/// Final mode (octal), `-` if unknown.
fn mode(a: &PlannedAction) -> String {
    a.mode
        .map_or_else(|| "-".to_owned(), |m| format!("{m:04o}"))
}

/// Mode and size for files, mode for directories, `-> target` for symlinks.
fn facts(a: &PlannedAction) -> String {
    match a.kind {
        ObjectKind::File => format!("{}  {}", mode(a), human_size(a.size)),
        ObjectKind::Dir => mode(a),
        ObjectKind::Symlink => format!("-> {}", safe(a.link_target.as_deref().unwrap_or("?"))),
    }
}

/// Cross-OS items' paths, source → this computer.
fn mappings(out: &mut impl Write, p: &Plan) -> io::Result<()> {
    if p.mappings.is_empty() {
        return Ok(());
    }
    writeln!(out, "Mapped to this OS's locations:")?;
    for PathMapping { item, from, to, .. } in &p.mappings {
        writeln!(
            out,
            "  {}  {}:{} -> {}:{}",
            safe(item),
            from.0,
            safe(&from.1),
            to.0,
            safe(&to.1)
        )?;
    }
    Ok(())
}

/// Conflicts, handlers, plugins (with their permissions) and the apps
/// the plan affects.
fn side_effects(out: &mut impl Write, p: &Plan) -> io::Result<()> {
    let conflicts = p.conflicts().count();
    writeln!(
        out,
        "Conflicts: {conflicts} (change with --rule ITEM|ROOT:REL|default=replace|keep-both|skip)"
    )?;
    for h in &p.handlers {
        let trust = if h.import_trust {
            "trust import on"
        } else {
            "trust import off (enable with --import-trust)"
        };
        writeln!(
            out,
            "Handler: {} runs `{}` (backs up {} path(s) first; {trust})",
            safe(&h.item),
            h.handler,
            h.backup.len()
        )?;
    }
    for plugin in &p.plugins {
        planned_plugin(out, plugin)?;
    }
    if !p.missing_apps.is_empty() {
        writeln!(
            out,
            "Apps not installed here: {}",
            safe(&p.missing_apps.join(", "))
        )?;
    }
    if !p.running_apps.is_empty() {
        writeln!(
            out,
            "Running apps that should be closed: {}",
            safe(&p.running_apps.join(", "))
        )?;
    }
    Ok(())
}

/// `Space: writes X`, then each filesystem's need and free space.
fn space(out: &mut impl Write, p: &Plan, needs: &[SpaceNeed]) -> io::Result<()> {
    let v = space_view(p, needs);
    writeln!(out, "Space: writes {}", human_size(v.writes))?;
    for fs in &v.filesystems {
        let free = match (fs.available, fs.total) {
            (Some(free), Some(total)) => format!("{} of {}", human_size(free), human_size(total)),
            (Some(free), None) => human_size(free),
            (None, _) => "unknown".to_owned(),
        };
        write!(
            out,
            "  on {}: needs about {}, free {free}",
            fs.what,
            human_size(fs.needed)
        )?;
        match fs.short_by {
            Some(short) => writeln!(out, "  (SHORT by {})", human_size(short))?,
            None => writeln!(out)?,
        }
    }
    Ok(())
}

/// Warnings, then blockers.
fn problems(out: &mut impl Write, p: &Plan) -> io::Result<()> {
    if !p.warnings.is_empty() {
        writeln!(out, "Warnings:")?;
        for w in &p.warnings {
            match &w.item {
                Some(item) => writeln!(out, "  - {}: {}", safe(item), safe(&w.message))?,
                None => writeln!(out, "  - {}", safe(&w.message))?,
            }
        }
    }
    if !p.blockers.is_empty() {
        writeln!(out, "Blocked:")?;
        for b in &p.blockers {
            write!(out, "  - {}", safe(&b.to_string()))?;
            match b {
                Blocker::CrossOs { item, .. } => {
                    write!(out, " (--allow-cross-os {})", safe(item))?;
                }
                Blocker::UnconfirmedPlugin { plugin, .. } => {
                    write!(out, " (--allow-plugin {plugin})")?;
                }
                _ => {}
            }
            writeln!(out)?;
        }
    }
    Ok(())
}

/// `Plugin: <id> <version> changes <items>: <description>`, then one line
/// per permission: what the user is asked to allow (`--allow-plugin`), on
/// either side.
pub fn planned_plugin(out: &mut impl Write, plugin: &PlannedPlugin) -> io::Result<()> {
    let items: Vec<&str> = plugin.items.iter().map(ItemKey::as_str).collect();
    writeln!(
        out,
        "Plugin: {} {} changes {}: {}",
        plugin.plugin,
        safe(&plugin.version),
        safe(&items.join(", ")),
        safe(&plugin.description)
    )?;
    for line in &plugin.permissions {
        writeln!(out, "    - {}", safe(line))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/render/plan/unit.rs"]
mod tests;

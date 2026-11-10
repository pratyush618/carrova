//! Text rendering of the export selection ([`carrova_view::selection`]).

use std::io::{self, Write};

use carrova_core::ItemKey;

use carrova_view::selection::{SelectionRow, SelectionTag, SelectionView};

use super::plan::planned_plugin;
use super::text::{human_size, safe};

/// The export selection: every recipe item (ticked or not) with its marks,
/// ticked raw dirs, a hint about the rest, the exporting plugins with
/// their permissions, and the problems.
pub fn selection(out: &mut impl Write, v: &SelectionView<'_>) -> io::Result<()> {
    let w = v
        .rows
        .iter()
        .map(|r| r.key.as_str().len())
        .max()
        .unwrap_or(0);
    writeln!(out, "Selection ([x] = exported):")?;
    for row in &v.rows {
        selection_row(out, row, w)?;
    }
    if v.hidden_raw > 0 {
        writeln!(
            out,
            "  ({} unrecognised dirs not selected; see `carrova scan`, add with --include raw:<root>/<rel>)",
            v.hidden_raw
        )?;
    }
    writeln!(
        out,
        "{} items, about {} before exclusions",
        v.included,
        human_size(v.included_size)
    )?;
    for plugin in v.plugins {
        planned_plugin(out, plugin)?;
    }
    if !v.issues.is_empty() {
        writeln!(out, "Problems:")?;
        for i in &v.issues {
            write!(out, "  - {}: {}", safe(&i.item), safe(&i.problem))?;
            if let Some(plugin) = &i.plugin {
                write!(out, " (--allow-plugin {plugin})")?;
            }
            writeln!(out)?;
        }
    }
    Ok(())
}

/// `  [x] key  size  files  label  [tags]`, then its excluded paths.
/// `w` is the key column width (of the unescaped keys).
fn selection_row(out: &mut impl Write, row: &SelectionRow<'_>, w: usize) -> io::Result<()> {
    let mark = if row.included { "x" } else { " " };
    let size = row.facts.map_or_else(String::new, |f| human_size(f.size));
    let files = row
        .facts
        .map_or_else(String::new, |f| format!("{} files", f.files));
    let key = safe(row.key);
    let label = safe(row.facts.map_or("", |f| f.label));
    write!(out, "  [{mark}] {key:<w$}  {size:>10}  {files:>9}  {label}")?;
    if !row.tags.is_empty() {
        let tags: Vec<String> = row.tags.iter().map(|t| tag(*t, row.key)).collect();
        write!(out, "  [{}]", tags.join("; "))?;
    }
    writeln!(out)?;
    for ex in row.excluded_paths {
        writeln!(out, "        excluded: {}", safe(ex))?;
    }
    Ok(())
}

fn tag(t: SelectionTag<'_>, key: &ItemKey) -> String {
    match t {
        SelectionTag::Suggested => "suggested".to_owned(),
        SelectionTag::Sensitive { confirmed: true } => "sensitive, confirmed".to_owned(),
        SelectionTag::Sensitive { confirmed: false } => {
            format!("sensitive: needs --confirm-sensitive {}", safe(key))
        }
        SelectionTag::Unmovable {
            reason,
            forced: true,
        } => format!("unmovable ({}), FORCED", safe(reason)),
        SelectionTag::Unmovable {
            reason,
            forced: false,
        } => format!("unmovable: {}", safe(reason)),
    }
}

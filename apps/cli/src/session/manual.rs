//! Manual mode of `export` and `send` (`carrova export PATH...`): the
//! given paths picked into an empty selection by the engine
//! ([`Exporter::pick`]), sensitive picks confirmed by flag or on the
//! terminal, never by `--yes`.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use carrova_core::select::{PickResult, Selection, SelectionEdit};
use carrova_core::{CoreError, Exporter, ItemKey};
use carrova_view::selection::{SelectionTag, selection_view};

use super::SelectionArgs;
use crate::output::OutputFormat;
use crate::prompt::{Consent, confirm};
use crate::render::selection::{pick_rejections, selection};
use crate::render::text::safe;

/// The selection step in Manual mode: pick `a`'s paths (any refusal fails
/// before anything is written), apply the allowed flags, confirm the
/// sensitive picks, show it (human output), hand it to the engine.
/// Returns the number of items selected.
pub(super) fn choose_manual(
    out: &mut impl Write,
    exporter: &mut Exporter,
    a: &SelectionArgs,
    format: OutputFormat,
) -> Result<usize> {
    let paths = a
        .paths()
        .iter()
        .map(|p| absolute(p))
        .collect::<Result<Vec<_>>>()?;
    let mut sel = Selection::default();
    let outcomes = exporter.pick(&mut sel, &paths);
    let rejected = outcomes
        .iter()
        .filter(|o| matches!(o.result, PickResult::Rejected { .. }))
        .count();
    if rejected > 0 {
        pick_rejections(&mut std::io::stderr().lock(), &outcomes)?;
        bail!("{rejected} of the paths cannot be picked; nothing was written");
    }
    let confirmed = a
        .confirm_sensitive()
        .iter()
        .map(|given| confirmed_pick(exporter, &sel, given))
        .collect::<Result<Vec<_>>>()?;
    let tick = confirmed.iter().map(|item| SelectionEdit::Item {
        item: item.clone(),
        included: true,
    });
    sel.apply_all(a.edits_confirming(&confirmed).chain(tick))
        .map_err(CoreError::from)?;
    confirm_sensitive_picks(&mut sel, exporter)?;
    if format == OutputFormat::Human {
        selection(out, &selection_view(&sel, exporter.scan()))?;
    }
    let n = sel.included().count();
    exporter.select(sel)?;
    Ok(n)
}

/// `p` made absolute against the working directory (not resolved: the
/// engine judges links itself).
fn absolute(p: &Path) -> Result<PathBuf> {
    std::path::absolute(p).with_context(|| format!("cannot resolve {}", p.display()))
}

/// The pick a `--confirm-sensitive` value names: its item key, or the
/// path it was picked by.
fn confirmed_pick(exporter: &Exporter, sel: &Selection, given: &ItemKey) -> Result<ItemKey> {
    if sel.items.contains_key(given) {
        return Ok(given.clone());
    }
    match exporter.pick_key(&absolute(Path::new(given.as_str()))?) {
        Ok(key) if sel.items.contains_key(&key) => Ok(key),
        _ => bail!(
            "--confirm-sensitive {} names nothing you picked",
            safe(given.as_str())
        ),
    }
}

/// Ask about each sensitive pick still unconfirmed (only on a terminal);
/// without one, fail naming the flag that confirms it.
fn confirm_sensitive_picks(sel: &mut Selection, exporter: &Exporter) -> Result<()> {
    let pending: Vec<(ItemKey, String)> = selection_view(sel, exporter.scan())
        .picks
        .into_iter()
        .filter(|p| {
            p.tags
                .contains(&SelectionTag::Sensitive { confirmed: false })
        })
        .map(|p| (p.key.clone(), p.display))
        .collect();
    let interactive = std::io::stdin().is_terminal();
    for (item, display) in pending {
        let display = safe(&display);
        if !interactive {
            bail!(
                "{display} looks like it holds keys or passwords; confirm it with \
                 --confirm-sensitive {} (--yes never does)",
                safe(item.as_str())
            );
        }
        let question = format!("Include {display}? It looks like it holds keys or passwords");
        if confirm(Consent::Ask, &question)? {
            sel.apply_all([
                SelectionEdit::ConfirmSensitive {
                    item: item.clone(),
                    confirmed: true,
                },
                SelectionEdit::Item {
                    item,
                    included: true,
                },
            ])
            .map_err(CoreError::from)?;
        }
    }
    Ok(())
}

//! `carrova export | import | rollback | sessions`, plus the selection and
//! plan/review/apply flows shared with `carrova send | receive`
//! ([`crate::lan`]). Each command only builds engine input (`Selection`,
//! `PlanOptions`), asks the user, calls the engine and renders its results.

mod args;
mod manual;
mod review;

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use carrova_core::engine::sessions;
use carrova_core::session::{EventSink, Role, SessionId};
use carrova_core::{
    ArchiveSummary, Cancel, Checklist, Engine, Exporter, Importer, KdfStrength, ScanOptions,
    WriteOptions,
};
use serde_json::json;

pub use args::{
    ExportArgs, ImportArgs, PlanArgs, RollbackArgs, SelectionArgs, SessionsArgs, SessionsCommand,
};
pub use review::review_and_apply;

use crate::events::CliSink;
use crate::interrupt;
use crate::output::{OutputFormat, print_json};
use crate::prompt::{Consent, confirm, ensure_can_confirm, passphrase};
use crate::recipes::{LoadRecipes, RecipeLocations, seams};
use crate::render::preview;
use crate::render::selection::selection;
use crate::render::session::{self as report, ChecklistOf};
use crate::render::text::human_size;
use carrova_platform::Platform;
use carrova_view::Detail;
use carrova_view::ipc::NearbySettings;
use carrova_view::passphrase::PassphraseUse;
use carrova_view::selection::selection_view;

/// Engine wired like `scan`: the resolved recipes and their plugins, the
/// current platform and the desktop app's Nearby settings (CLI = app).
pub fn engine(recipes: &RecipeLocations, sink: Arc<dyn EventSink>) -> Result<Engine> {
    let (recipes, plugins) = recipes.recipes_and_plugins()?;
    let platform: Arc<dyn Platform> = Arc::from(seams::platform()?);
    let nearby = NearbySettings::saved(platform.as_ref());
    Ok(Engine::new(platform, recipes, sink)
        .with_plugins(plugins)
        .with_remember_devices(nearby.remember)
        .with_wol_hints(nearby.wol_hints))
}

/// `carrova export`
pub fn export(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    verbose: u8,
    a: &ExportArgs,
) -> Result<ExitCode> {
    ensure_can_confirm(a.consent(), "export")?;
    let strength = a.kdf_strength.map(KdfStrength::from).unwrap_or_default();
    let mut write_opts = WriteOptions::from(strength);
    if let Some(mib) = a.kdf_memory {
        write_opts.kdf.m_kib = mib
            .checked_mul(1024)
            .ok_or_else(|| anyhow!("--kdf-memory {mib} MiB is too large"))?;
    }
    let format = OutputFormat::from_json_flag(a.json);
    let engine = engine(recipes, Arc::new(CliSink::new(format, verbose)))?;
    let mut exporter = start_exporter(engine, format, a.selection.is_manual())?;
    let n = choose(out, &mut exporter, &a.selection, format)?;
    if !confirm(
        a.consent(),
        &format!("Export {n} items to {}?", a.output.display()),
    )? {
        eprintln!("export cancelled; nothing written");
        return Ok(ExitCode::FAILURE);
    }

    let pass = passphrase(PassphraseUse::NewArchive)?;
    let (summary, checklist) = {
        let _ctrl_c = interrupt::arm(&exporter.canceller(), format);
        exporter.export(&a.output, &pass, write_opts)?
    };
    drop(pass);
    exported(out, exporter.id(), &a.output, &summary, &checklist, format)?;
    Ok(ExitCode::SUCCESS)
}

/// Start `export` and `send`, cancellable with Ctrl-C: with the app scan,
/// or without it in Manual mode (`manual`, paths given).
pub fn start_exporter(engine: Engine, format: OutputFormat, manual: bool) -> Result<Exporter> {
    let cancel = Cancel::default();
    let _ctrl_c = interrupt::arm(&cancel, format);
    Ok(if manual {
        Exporter::start_manual(engine, cancel)?
    } else {
        Exporter::start_with(engine, ScanOptions::default(), cancel)?
    })
}

/// The export result: the archive written and the checklist.
fn exported(
    out: &mut impl Write,
    id: &SessionId,
    archive: &Path,
    summary: &ArchiveSummary,
    list: &Checklist,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(
            out,
            &json!({
                "session": id,
                "archive": archive,
                "bytes": summary.bytes,
                "chunks": summary.chunks,
                "checklist": list,
            }),
        ),
        OutputFormat::Human => {
            writeln!(
                out,
                "Wrote {} ({}, {} chunks), session {id}",
                archive.display(),
                human_size(summary.bytes),
                summary.chunks
            )?;
            let of = ChecklistOf::Local {
                session: &id.0,
                role: Role::Export,
            };
            report::checklist(out, list, of)?;
            Ok(())
        }
    }
}

/// The selection step of `export` and `send`: the suggested pre-fill,
/// the user's edits from `a`, shown for review (human output only), handed
/// to the engine; the picked paths instead in Manual mode. Returns the
/// number of items selected.
pub fn choose(
    out: &mut impl Write,
    exporter: &mut Exporter,
    a: &SelectionArgs,
    format: OutputFormat,
) -> Result<usize> {
    if a.is_manual() {
        return manual::choose_manual(out, exporter, a, format);
    }
    let mut sel = exporter.suggest(a.preset());
    a.edit(&mut sel)?;
    if format == OutputFormat::Human {
        selection(out, &selection_view(&sel, exporter.scan()))?;
    }
    let n = sel.included().count();
    exporter.select(sel)?;
    Ok(n)
}

/// `carrova import`
pub fn import(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    verbose: u8,
    a: &ImportArgs,
) -> Result<ExitCode> {
    let p = &a.plan;
    if p.needs_confirmation() {
        ensure_can_confirm(p.consent(), "import")?;
    }
    let engine = engine(recipes, Arc::new(CliSink::new(p.format(), verbose)))?;
    let platform = Arc::clone(engine.platform());
    let pass = passphrase(PassphraseUse::ExistingArchive)?;
    let mut importer = Importer::open(engine, &a.file, &pass)?;
    drop(pass);
    review_and_apply(out, &mut importer, platform.as_ref(), p, verbose)
}

/// `carrova rollback`
pub fn rollback(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    verbose: u8,
    a: &RollbackArgs,
) -> Result<ExitCode> {
    let format = OutputFormat::from_json_flag(a.json);
    let engine = engine(recipes, Arc::new(CliSink::new(format, verbose)))?;
    if a.preview {
        let p = sessions::rollback_preview(&engine, a.session.as_ref())?;
        match format {
            OutputFormat::Json => print_json(out, &serde_json::to_value(&p)?)?,
            OutputFormat::Human => preview::rollback_preview(out, &p)?,
        }
        return Ok(ExitCode::SUCCESS);
    }
    let consent = a.consent();
    ensure_can_confirm(consent, "rollback")?;
    let what = a.session.as_ref().map_or_else(
        || "the latest import".to_owned(),
        |id| format!("import session {id}"),
    );
    let question = if consent == Consent::Assume {
        String::new()
    } else {
        let p = sessions::rollback_preview(&engine, a.session.as_ref())?;
        format!("Roll back {what}? It would do: {}.", preview::counts(&p))
    };
    if !confirm(consent, &question)? {
        eprintln!("rollback cancelled");
        return Ok(ExitCode::FAILURE);
    }
    let r = sessions::rollback(&engine, a.session.as_ref())?;
    match format {
        // Through a `Value`: its keys are sorted, the established shape.
        OutputFormat::Json => print_json(out, &serde_json::to_value(&r)?)?,
        OutputFormat::Human => report::rollback(out, &r, Detail::from_verbosity(verbose))?,
    }
    Ok(if r.failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// `carrova sessions`
pub fn sessions(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    verbose: u8,
    a: SessionsArgs,
) -> Result<ExitCode> {
    let command = a.command.unwrap_or(SessionsCommand::List);
    if let SessionsCommand::Dismiss { yes, .. } = &command {
        ensure_can_confirm(*yes, "dismiss")?;
    }
    let format = OutputFormat::from_json_flag(a.json);
    let engine = engine(recipes, Arc::new(CliSink::new(format, verbose)))?;
    match command {
        SessionsCommand::List => {
            let list = sessions::list(&engine)?;
            match format {
                OutputFormat::Json => print_json(out, &serde_json::to_value(&list)?)?,
                OutputFormat::Human => report::session_list(out, &list)?,
            }
        }
        SessionsCommand::Show { id } => {
            let s = sessions::show(&engine, &id)?;
            match format {
                OutputFormat::Json => print_json(out, &serde_json::to_value(&s)?)?,
                OutputFormat::Human => report::session_show(out, &s)?,
            }
        }
        SessionsCommand::Dismiss { id, yes } => {
            return dismiss(out, &engine, &id, yes.into(), format);
        }
        SessionsCommand::ExportLog { id, output } => {
            export_log(out, &engine, &id, &output, format)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `carrova sessions dismiss`
fn dismiss(
    out: &mut impl Write,
    engine: &Engine,
    id: &SessionId,
    consent: Consent,
    format: OutputFormat,
) -> Result<ExitCode> {
    if !confirm(
        consent,
        &format!("Dismiss session {id}? Its rollback point is deleted; this cannot be undone."),
    )? {
        eprintln!("dismiss cancelled");
        return Ok(ExitCode::FAILURE);
    }
    sessions::dismiss(engine, id)?;
    match format {
        OutputFormat::Json => print_json(out, &json!({ "dismissed": id }))?,
        OutputFormat::Human => writeln!(out, "Dismissed session {id}.")?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `carrova sessions export-log`
fn export_log(
    out: &mut impl Write,
    engine: &Engine,
    id: &SessionId,
    output: &Path,
    format: OutputFormat,
) -> Result<()> {
    sessions::export_log(engine, id, output)
        .with_context(|| format!("exporting log of session {id}"))?;
    match format {
        OutputFormat::Json => print_json(out, &json!({ "session": id, "log": output })),
        OutputFormat::Human => {
            writeln!(out, "Wrote log of session {id} to {}.", output.display())?;
            Ok(())
        }
    }
}

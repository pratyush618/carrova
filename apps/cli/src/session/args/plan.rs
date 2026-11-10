//! `carrova import` and the plan / review / apply flags it shares with
//! `carrova receive`, turned into engine [`PlanOptions`].

use std::path::PathBuf;

use carrova_core::ItemKey;
use carrova_core::plan::{PlanOptions, Retention};
use carrova_recipe::PluginId;
use clap::Args;

use super::FailpointArgs;
use crate::args::{RemapArg, RuleArg, RuleTarget, parse_remap, parse_rule};
use crate::output::OutputFormat;
use crate::prompt::Consent;

/// `carrova import`
#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Archive file to import.
    pub(crate) file: PathBuf,
    #[command(flatten)]
    pub(crate) plan: PlanArgs,
}

/// How to plan, review and apply an import: shared by `carrova import` and
/// `carrova receive`.
#[derive(Debug, Args)]
pub struct PlanArgs {
    /// Only show the plan; write nothing.
    #[arg(long, conflicts_with = "yes")]
    dry_run: bool,
    /// Import only these offered items (repeatable; default: every
    /// non-sensitive item plus confirmed sensitive ones).
    #[arg(long, value_name = "KEY")]
    item: Vec<ItemKey>,
    /// Explicitly confirm importing a sensitive item.
    #[arg(long, value_name = "ITEM")]
    confirm_sensitive: Vec<ItemKey>,
    /// Restore a same-OS item although the source is another OS family
    /// (the app may not read it here; warned).
    #[arg(long, value_name = "ITEM")]
    allow_cross_os: Vec<ItemKey>,
    /// Allow plugin ID to change the files of the items that use it, after
    /// reviewing its permissions in the plan (repeatable).
    #[arg(long, value_name = "ID")]
    allow_plugin: Vec<PluginId>,
    /// Conflict rule: `default|ITEM|ROOT:REL|ITEM:ROOT:REL=replace|keep-both|skip`.
    /// File rules beat item rules beat `default` (suggested: replace).
    #[arg(long, value_name = "TARGET=RULE", value_parser = parse_rule)]
    rule: Vec<RuleArg>,
    /// Restore an item elsewhere: `ITEM=ROOT`, `ITEM=ROOT:PREFIX` or `ITEM=:PREFIX`.
    #[arg(long, value_name = "ITEM=ROOT[:PREFIX]", value_parser = parse_remap)]
    remap: Vec<RemapArg>,
    /// Apply even though an app that should be closed is running (warned).
    #[arg(long)]
    ignore_running: bool,
    /// Apply even though free space looks insufficient (warned).
    #[arg(long)]
    ignore_free_space: bool,
    /// Name suffix for keep-both copies; `{date}` becomes YYYYMMDD.
    #[arg(long, value_name = "S")]
    keep_both_suffix: Option<String>,
    /// Keep group-write bits from the source (e.g. 0664 files under a
    /// umask-002 setup). World-write, setuid and setgid are never restored.
    #[arg(long)]
    keep_group_write: bool,
    /// Also import gpg ownertrust from the source. Off by default: an
    /// archive or sender could mark an attacker's key as trusted.
    #[arg(long)]
    import_trust: bool,
    /// Discard the rollback point right after a successful apply.
    #[arg(long)]
    discard_rollback: bool,
    /// Don't ask for confirmation (required when stdin is not a terminal).
    #[arg(short, long)]
    yes: bool,
    /// Events as JSON lines on stderr; final result as JSON on stdout.
    #[arg(long)]
    json: bool,
    #[command(flatten)]
    test: FailpointArgs,
}

impl PlanArgs {
    /// Confirmation will be needed (not a dry run).
    pub fn needs_confirmation(&self) -> bool {
        !self.dry_run
    }

    /// `--dry-run`: show the plan, write nothing.
    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    /// `--yes`: ask, or agreed up front.
    pub fn consent(&self) -> Consent {
        self.yes.into()
    }

    /// `--json`.
    pub fn format(&self) -> OutputFormat {
        OutputFormat::from_json_flag(self.json)
    }

    /// Arm the test-only fault injection (`--fail-after`) on `importer`.
    #[cfg(feature = "failpoints")]
    pub fn arm_failpoint(&self, importer: &mut carrova_core::Importer) {
        self.test.arm(importer);
    }

    /// `base` with the choices these flags make: a new session's
    /// defaults, or a resumed session's own options
    /// ([`carrova_core::Importer::plan_options`]). Only flags the user
    /// passed change anything: a list flag (`--item`,
    /// `--confirm-sensitive`, `--allow-cross-os`, `--allow-plugin`)
    /// replaces that list, `--rule` / `--remap` set their target, a switch
    /// turns its option on. So a resume without
    /// flags keeps the approved options, and with them the approval.
    pub fn options_over(&self, base: PlanOptions) -> PlanOptions {
        let mut o = base;
        if !self.item.is_empty() {
            o.items = Some(self.item.clone());
        }
        if !self.confirm_sensitive.is_empty() {
            o.confirmed_sensitive.clone_from(&self.confirm_sensitive);
        }
        if !self.allow_cross_os.is_empty() {
            o.allow_cross_os.clone_from(&self.allow_cross_os);
        }
        if !self.allow_plugin.is_empty() {
            o.confirmed_plugins.clone_from(&self.allow_plugin);
        }
        for r in &self.rule {
            match &r.target {
                RuleTarget::Default => o.default_rule = r.rule,
                RuleTarget::Item(k) => {
                    o.item_rules.insert(k.clone(), r.rule);
                }
                RuleTarget::Path(k) => {
                    o.path_rules.insert(k.clone(), r.rule);
                }
            }
        }
        for r in &self.remap {
            o.remaps.insert(r.item.clone(), r.remap.clone());
        }
        o.ignore_requires_closed |= self.ignore_running;
        o.ignore_free_space |= self.ignore_free_space;
        o.import_trust |= self.import_trust;
        o.keep_group_write |= self.keep_group_write;
        if let Some(s) = &self.keep_both_suffix {
            o.keep_both_suffix.clone_from(s);
        }
        if self.discard_rollback {
            o.retention = Retention::DiscardOnSuccess;
        }
        o
    }
}

#[cfg(test)]
#[path = "../../../tests/session/args/unit.rs"]
mod tests;

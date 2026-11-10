//! Syntax of the composite flag values (`--rule`, `--remap`,
//! `--exclude-path`) and of the receiver's pairing ticket. Parsing only turns text into the engine's data types;
//! whether keys exist, paths are confined, etc. is decided by the engine.

use carrova_core::plan::{ConflictRule, Remap};
use carrova_core::{ItemKey, RootedPath};
use carrova_platform::SemanticRoot;

/// What a `--rule` applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleTarget {
    /// `default=…`: every conflict without a more specific rule.
    Default,
    /// `ITEM=…`: every conflict of one archive item.
    Item(ItemKey),
    /// `ROOT:REL=…` or `ITEM:ROOT:REL=…`: one target file (`"<root>:<rel>"`).
    Path(String),
}

/// One `--rule TARGET=RULE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleArg {
    pub target: RuleTarget,
    pub rule: ConflictRule,
}

/// One `--remap ITEM=ROOT[:PREFIX]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemapArg {
    pub item: ItemKey,
    pub remap: Remap,
}

/// One `--exclude-path ITEM=ROOT:REL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathExclusion {
    pub item: ItemKey,
    /// `"<root>:<rel>"`.
    pub path: String,
}

fn roots() -> String {
    SemanticRoot::ALL
        .iter()
        .map(|r| r.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_root(s: &str) -> Result<SemanticRoot, String> {
    SemanticRoot::ALL
        .into_iter()
        .find(|r| r.as_str() == s)
        .ok_or_else(|| format!("unknown root `{s}` (expected one of: {})", roots()))
}

fn split_eq<'a>(s: &'a str, syntax: &str) -> Result<(&'a str, &'a str), String> {
    match s.split_once('=') {
        Some((l, r)) if !l.is_empty() && !r.is_empty() => Ok((l, r)),
        _ => Err(format!("expected {syntax}")),
    }
}

/// `replace` | `keep-both` | `skip`.
pub fn parse_conflict_rule(s: &str) -> Result<ConflictRule, String> {
    match s {
        "replace" => Ok(ConflictRule::Replace),
        "keep-both" => Ok(ConflictRule::KeepBoth),
        "skip" => Ok(ConflictRule::Skip),
        _ => Err(format!(
            "unknown rule `{s}` (expected replace, keep-both or skip)"
        )),
    }
}

/// `[default|ITEM|ROOT:REL|ITEM:ROOT:REL]=replace|keep-both|skip`.
pub fn parse_rule(s: &str) -> Result<RuleArg, String> {
    const SYNTAX: &str = "default=RULE, ITEM=RULE, ROOT:REL=RULE or ITEM:ROOT:REL=RULE";
    let (target, rule) = s
        .rsplit_once('=')
        .ok_or_else(|| format!("expected {SYNTAX}"))?;
    if target.is_empty() {
        return Err(format!("expected {SYNTAX}"));
    }
    let rule = parse_conflict_rule(rule)?;
    let target = if target == "default" {
        RuleTarget::Default
    } else if RootedPath::parse(target).is_some() {
        RuleTarget::Path(target.to_owned())
    } else if let Some(path) = item_path_suffix(target) {
        RuleTarget::Path(path.to_owned())
    } else if target.starts_with(ItemKey::RAW_PREFIX) || !target.contains(':') {
        RuleTarget::Item(target.into())
    } else {
        return Err(format!(
            "`{target}` is neither an item key nor ROOT:REL (roots: {})",
            roots()
        ));
    };
    Ok(RuleArg { target, rule })
}

/// In `ITEM:ROOT:REL`, the `ROOT:REL` part (rules are keyed by target path;
/// the item prefix only documents where the file comes from).
fn item_path_suffix(s: &str) -> Option<&str> {
    SemanticRoot::ALL.into_iter().find_map(|root| {
        let needle = format!(":{}:", root.as_str());
        let at = s.find(&needle)?;
        let path = &s[at + 1..];
        (at > 0 && RootedPath::parse(path).is_some()).then_some(path)
    })
}

/// `ITEM=ROOT`, `ITEM=ROOT:PREFIX` or `ITEM=:PREFIX` (keep root).
pub fn parse_remap(s: &str) -> Result<RemapArg, String> {
    let (item, target) = split_eq(s, "ITEM=ROOT[:PREFIX] or ITEM=:PREFIX")?;
    let (root, prefix) = match target.split_once(':') {
        Some((root, prefix)) => (root, Some(prefix)),
        None => (target, None),
    };
    let root = if root.is_empty() {
        None
    } else {
        Some(parse_root(root)?)
    };
    let rel_prefix = match prefix {
        Some("") => return Err("remap prefix after `:` is empty".into()),
        Some(p) => Some(p.to_owned()),
        None => None,
    };
    if root.is_none() && rel_prefix.is_none() {
        return Err("remap needs a root and/or a prefix".into());
    }
    Ok(RemapArg {
        item: item.into(),
        remap: Remap { root, rel_prefix },
    })
}

/// `ITEM=ROOT:REL`.
pub fn parse_exclude_path(s: &str) -> Result<PathExclusion, String> {
    let (item, path) = split_eq(s, "ITEM=ROOT:REL")?;
    if RootedPath::parse(path).is_none() {
        return Err(format!("`{path}` is not ROOT:REL (roots: {})", roots()));
    }
    Ok(PathExclusion {
        item: item.into(),
        path: path.to_owned(),
    })
}

/// `carrova sessions`/`rollback` id: shape-checked by the engine's rule.
pub fn parse_session_id(s: &str) -> Result<carrova_core::session::SessionId, String> {
    if carrova_core::session::SessionId::is_valid(s) {
        Ok(carrova_core::session::SessionId(s.to_owned()))
    } else {
        Err(format!(
            "`{s}` is not a session id (see `carrova sessions list`)"
        ))
    }
}

/// `<code>` or `<code>@<ipv4>:<port>`, as shown on the sending machine.
/// The error never repeats the input: a near-miss is still most of a
/// secret code.
pub fn parse_ticket(s: &str) -> Result<carrova_core::Ticket, String> {
    s.parse().map_err(|e| {
        format!(
            "not a pairing code ({e}); type it as shown on the sending machine, \
             e.g. 7-word-word or 7-word-word@192.168.1.20:47822"
        )
    })
}

/// Clap parser for the `carrova receive` code argument via
/// [`parse_ticket`]. Unlike a plain `value_parser = fn`, its errors do not
/// echo the typed value.
#[derive(Debug, Clone, Copy)]
pub struct TicketParser;

impl clap::builder::TypedValueParser for TicketParser {
    type Value = carrova_core::Ticket;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        value
            .to_str()
            .ok_or_else(|| "not a pairing code (not valid UTF-8)".to_owned())
            .and_then(parse_ticket)
            .map_err(|msg| {
                clap::Error::raw(clap::error::ErrorKind::ValueValidation, format!("{msg}\n"))
                    .with_cmd(cmd)
            })
    }
}

#[cfg(test)]
#[path = "../tests/args/unit.rs"]
mod tests;

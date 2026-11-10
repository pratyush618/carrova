//! App detection.

use std::path::PathBuf;

use carrova_platform::{FileKind, Platform};
use tracing::debug;

use crate::{DetectRule, PathRef, Recipe};

/// Outcome of [`detect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// At least one rule matched.
    pub detected: bool,
    /// Every rule that matched, in recipe order.
    pub evidence: Vec<Evidence>,
}

/// Why an app counts as present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// `find_binary(name)` returned `path`.
    Binary { name: String, path: PathBuf },
    /// The path exists with this (lstat) kind.
    Path { path: PathRef, kind: FileKind },
}

/// Evaluate the recipe's rules for the platform's OS. Any one match =
/// detected. Platform errors on a rule count as "no match" (logged at debug).
#[must_use]
pub fn detect(recipe: &Recipe, platform: &dyn Platform) -> Detection {
    let evidence: Vec<Evidence> = recipe
        .detect
        .rules(platform.os())
        .iter()
        .filter_map(|r| check(r, platform))
        .collect();
    Detection {
        detected: !evidence.is_empty(),
        evidence,
    }
}

fn check(rule: &DetectRule, platform: &dyn Platform) -> Option<Evidence> {
    match rule {
        DetectRule::Binary(name) => match platform.find_binary(name) {
            Ok(Some(path)) => Some(Evidence::Binary {
                name: name.clone(),
                path,
            }),
            Ok(None) => None,
            Err(e) => {
                debug!(binary = %name, error = %e, "binary lookup failed");
                None
            }
        },
        DetectRule::Path(pr) => {
            let root = match platform.resolve_root(pr.root) {
                Ok(r) => r,
                Err(e) => {
                    debug!(path = %pr, error = %e, "root unavailable");
                    return None;
                }
            };
            match platform.metadata(&pr.rel.join_to(&root)) {
                Ok(meta) => Some(Evidence::Path {
                    path: pr.clone(),
                    kind: meta.kind,
                }),
                Err(_) => None,
            }
        }
    }
}

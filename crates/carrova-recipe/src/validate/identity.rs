//! Recipe-level checks: identity, detection rules, process names.

use super::{IssueKind, Sink, is_bare_name, is_kebab};
use crate::{DetectRule, Recipe, SUPPORTED_SCHEMA};

/// Schema, id, name, category, and (for a loaded tree) the directory names.
pub(super) fn check_identity(r: &Recipe, layout: Option<&crate::TreeLayout>, s: &mut Sink<'_>) {
    if !SUPPORTED_SCHEMA.contains(&r.schema) {
        s.push("schema", IssueKind::UnsupportedSchema { found: r.schema });
    }
    if !is_kebab(&r.id) {
        s.push(
            "id",
            IssueKind::NotKebab {
                value: r.id.clone(),
            },
        );
    }
    if r.name.trim().is_empty() {
        s.push("name", IssueKind::Empty);
    }
    if !is_kebab(&r.category) {
        s.push(
            "category",
            IssueKind::NotKebab {
                value: r.category.clone(),
            },
        );
    }
    let Some(layout) = layout else {
        return;
    };
    if layout.category_dir != r.category {
        s.push(
            "category",
            IssueKind::CategoryDirMismatch {
                category: r.category.clone(),
                dir: layout.category_dir.clone(),
            },
        );
    }
    if layout.app_dir != r.id {
        s.push(
            "id",
            IssueKind::AppDirMismatch {
                id: r.id.clone(),
                dir: layout.app_dir.clone(),
            },
        );
    }
}

/// Detection rules: Linux needs at least one; binary names are bare.
pub(super) fn check_detect(r: &Recipe, s: &mut Sink<'_>) {
    if r.detect.linux.is_empty() {
        s.push("detect.linux", IssueKind::NoLinuxDetect);
    }
    for (os, rules) in [
        ("linux", &r.detect.linux),
        ("windows", &r.detect.windows),
        ("macos", &r.detect.macos),
    ] {
        for (i, rule) in rules.iter().enumerate() {
            if let DetectRule::Binary(name) = rule
                && !is_bare_name(name)
            {
                s.push(
                    format!("detect.{os}[{i}].binary"),
                    IssueKind::InvalidBinaryName { name: name.clone() },
                );
            }
        }
    }
}

/// `processes`: schema 2+, bare names.
pub(super) fn check_processes(r: &Recipe, s: &mut Sink<'_>) {
    if r.schema < 2 && !r.processes.is_empty() {
        s.push(
            "processes",
            IssueKind::RequiresSchema {
                field: "processes",
                min: 2,
            },
        );
    }
    for (i, name) in r.processes.iter().enumerate() {
        if !is_bare_name(name) {
            s.push(
                format!("processes[{i}]"),
                IssueKind::InvalidProcessName { name: name.clone() },
            );
        }
    }
}

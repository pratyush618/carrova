//! Recipe-level checks: identity, detection rules, process names.

use carrova_platform::OsKind;

use super::{IssueKind, Sink, is_bare_name, is_kebab, since};
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

/// Detection rules: before schema 3 Linux needs at least one, from schema
/// 3 at least one OS does; binary names are bare.
pub(super) fn check_detect(r: &Recipe, s: &mut Sink<'_>) {
    if r.schema < since::ANY_OS_DETECT {
        if !r.supports(OsKind::Linux) {
            s.push("detect.linux", IssueKind::NoLinuxDetect);
        }
    } else if !OsKind::ALL.into_iter().any(|os| r.supports(os)) {
        s.push("detect", IssueKind::NoDetect);
    }
    for os in OsKind::ALL {
        let os_id = os.as_str();
        for (i, rule) in r.detect.rules(os).iter().enumerate() {
            if let DetectRule::Binary(name) = rule
                && !is_bare_name(name)
            {
                s.push(
                    format!("detect.{os_id}[{i}].binary"),
                    IssueKind::InvalidBinaryName { name: name.clone() },
                );
            }
        }
    }
}

/// `processes`: schema 2+, bare names.
pub(super) fn check_processes(r: &Recipe, s: &mut Sink<'_>) {
    if !r.processes.is_empty() {
        s.require_schema(r.schema, "processes", "processes", since::PROCESSES);
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

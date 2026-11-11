//! Records the IPC fixtures the UI's fake port replays (`src/ipc/fixtures`):
//! real bridge results and events of an archive migration with the
//! repository's recipes over fake homes, temp paths replaced by stable
//! ones. Each fixture is a TypeScript module exporting one constant typed
//! with its generated binding, so `tsc` checks it against the contract.
//! Ignored by default (it writes files): `just ipc-fixtures`.

use std::path::{Path, PathBuf};

use carrova_core::{ConflictRule, ItemKey, KdfStrength, PlanOptions, Remap, SelectionEdit};
use carrova_platform::SemanticRoot;
use serde::Serialize;
use serde_json::Value;
use tauri::async_runtime::block_on;

use crate::ops::{receive, recipes, send, sessions};
use crate::testing::{Machine, PASS};

/// Apps installed on both fake machines.
const BINARIES: &[&str] = &["bash", "git", "nvim", "ssh", "starship", "tmux"];

/// Where the fixtures go.
fn out_dir() -> PathBuf {
    std::env::var_os("CARROVA_IPC_FIXTURES_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/fixtures"),
        PathBuf::from,
    )
}

/// A machine using the repository's recipes.
fn machine(device: &str) -> Machine {
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../recipes");
    Machine::with_recipes(tempfile::tempdir().unwrap(), tree, BINARIES, Some(device))
}

/// Writes fixtures with `machine`'s temp paths replaced by stable ones.
struct Recorder {
    dir: PathBuf,
    replacements: Vec<(String, String)>,
}

impl Recorder {
    fn new(machines: &[(&Machine, &str)]) -> Self {
        let mut replacements = Vec::new();
        for (machine, user) in machines {
            let root = machine.root().display().to_string();
            replacements.push((format!("{root}/home"), format!("/home/{user}")));
            replacements.push((
                format!("{root}/state/carrova"),
                format!("/home/{user}/.local/state/carrova"),
            ));
            replacements.push((root, format!("/home/{user}/Downloads")));
        }
        let dir = out_dir();
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir, replacements }
    }

    /// Write `<name>.ts`: `export const <name>: <ty> = <value>;`, where
    /// `ty` is a binding type, optionally an array (`BridgeEvent[]`).
    fn write(&self, name: &str, ty: &str, value: &impl Serialize) {
        let mut value = serde_json::to_value(value).unwrap();
        as_js_numbers(&mut value);
        let mut json = serde_json::to_string_pretty(&value).unwrap();
        for (from, to) in &self.replacements {
            json = json.replace(from, to);
        }
        let binding = ty.trim_end_matches("[]");
        let constant = camel_case(name);
        let text = format!(
            "// Recorded by `just ipc-fixtures`; do not edit.\n\
             import type {{ {binding} }} from '../../bindings/{binding}';\n\n\
             export const {constant}: {ty} = {json};\n"
        );
        std::fs::write(self.dir.join(format!("{name}.ts")), text).unwrap();
    }
}

/// Round integers beyond 2^53 (nanosecond mtimes) to the nearest double,
/// which is what JavaScript holds after parsing them anyway.
fn as_js_numbers(value: &mut Value) {
    match value {
        Value::Number(n) => {
            if let Some(big) = n.as_u64().filter(|&u| u > 1 << 53) {
                #[expect(clippy::cast_precision_loss, reason = "exactly the JS rounding")]
                let rounded = big as f64;
                if let Some(exact) = serde_json::Number::from_f64(rounded) {
                    *n = exact;
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(as_js_numbers),
        Value::Object(fields) => fields.values_mut().for_each(as_js_numbers),
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
}

/// `export-events` → `exportEvents`.
fn camel_case(name: &str) -> String {
    let mut parts = name.split('-');
    let first = parts.next().unwrap_or_default().to_owned();
    parts.fold(first, |mut out, part| {
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            out.extend(c.to_uppercase());
            out.push_str(chars.as_str());
        }
        out
    })
}

/// Fill a developer's home.
fn populate(machine: &Machine) {
    for (rel, text) in [
        (
            ".gitconfig",
            "[user]\n\tname = Ana\n\temail = ana@example.org\n",
        ),
        (".bashrc", "export EDITOR=nvim\nalias ll='ls -la'\n"),
        (".ssh/config", "Host build\n\tHostName 10.0.0.7\n"),
        (
            ".ssh/id_ed25519",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nfake\n",
        ),
        (".ssh/id_ed25519.pub", "ssh-ed25519 AAAAfake ana@laptop\n"),
        (".config/nvim/init.lua", "vim.o.number = true\n"),
        (".config/nvim/lua/plugins.lua", "return {}\n"),
        (".config/starship.toml", "add_newline = false\n"),
        (".tmux.conf", "set -g mouse on\n"),
    ] {
        machine.put(rel, text.as_bytes());
    }
}

#[test]
#[ignore = "writes files: run through `just ipc-fixtures`"]
fn record_ipc_fixtures() {
    let sender = machine("ana-desktop");
    let receiver = machine("ana-laptop");
    populate(&sender);
    receiver.put(".gitconfig", b"[user]\n\tname = Ana (new laptop)\n");
    let rec = Recorder::new(&[(&sender, "ana"), (&receiver, "ana")]);

    let scan = block_on(send::scan(&sender.state)).unwrap();
    rec.write("scan", "ScanResult", &scan);
    let id = scan.result().session;
    rec.write(
        "catalog",
        "CatalogEntry[]",
        &block_on(recipes::catalog(&sender.state)).unwrap(),
    );
    rec.write(
        "recipes-status",
        "RecipesStatus",
        &block_on(recipes::status(&sender.state)).unwrap(),
    );

    let suggested = scan.result().selection.clone();
    let edits = more_edits();
    rec.write("select-edits-input", "SelectionEdit[]", &edits);
    let edited = block_on(send::select(&sender.state, &id, suggested.clone(), edits)).unwrap();
    rec.write("select-edits", "ScanResult", &edited);

    let keys = ItemKey::from("ssh/keys");
    let tick = SelectionEdit::Item {
        item: keys.clone(),
        included: true,
    };
    let refused = block_on(send::select(&sender.state, &id, suggested, vec![tick])).unwrap();
    rec.write("select-issues", "ScanResult", &refused);
    let confirm = SelectionEdit::ConfirmSensitive {
        item: keys,
        confirmed: true,
    };
    let base = refused.result().selection.clone();
    let accepted = block_on(send::select(&sender.state, &id, base, vec![confirm])).unwrap();
    rec.write("select", "ScanResult", &accepted);
    let archive = sender.root().join("laptop.carrova");
    let summary = block_on(send::export_archive(
        &sender.state,
        &id,
        archive.clone(),
        PASS.into(),
        KdfStrength::default(),
    ))
    .unwrap();
    rec.write("archive-summary", "ArchiveSummary", &summary);
    rec.write(
        "export-events",
        "BridgeEvent[]",
        &sender.events.recorded.events(),
    );
    record_manual(&rec, &sender);

    let opened = block_on(receive::open_archive(&receiver.state, archive, PASS.into())).unwrap();
    // First look: nothing confirmed yet, so sensitive items wait for the user.
    let first = block_on(receive::plan(
        &receiver.state,
        &opened.session,
        PlanOptions::default(),
    ))
    .unwrap();
    rec.write("plan-unconfirmed", "PlanReview", &first);
    let options = PlanOptions {
        // Receiver-side sensitivity also flags shell and git configs.
        confirmed_sensitive: ["ssh/keys", "ssh/config", "git/config", "bash/config"]
            .map(Into::into)
            .to_vec(),
        default_rule: ConflictRule::default(),
        ..PlanOptions::default()
    };
    let variants: Vec<_> = plan_variants(&options)
        .into_iter()
        .map(|o| block_on(receive::plan(&receiver.state, &opened.session, o)).unwrap())
        .collect();
    rec.write("plan-variants", "PlanReview[]", &variants);
    let review = block_on(receive::plan(&receiver.state, &opened.session, options)).unwrap();
    rec.write("plan", "PlanReview", &review);
    block_on(receive::approve(&receiver.state, &opened.session)).unwrap();
    let checklist = block_on(receive::apply(&receiver.state, &opened.session)).unwrap();
    rec.write("checklist", "Checklist", &checklist);
    rec.write(
        "import-events",
        "BridgeEvent[]",
        &receiver.events.recorded.events(),
    );
    rec.write(
        "sessions",
        "SessionRecord[]",
        &block_on(sessions::list(&receiver.state)).unwrap(),
    );
    let preview = block_on(sessions::rollback_preview(
        &receiver.state,
        Some(opened.session.clone()),
    ))
    .unwrap();
    rec.write("rollback-preview", "RollbackPreview", &preview);
    let rollback = block_on(sessions::rollback(&receiver.state, Some(opened.session))).unwrap();
    rec.write("rollback", "RollbackReport", &rollback);
}

/// A Manual session on `sender`: its start, then picks that are added,
/// absorbed, sensitive and refused.
fn record_manual(rec: &Recorder, sender: &Machine) {
    let manual = block_on(send::start_manual(&sender.state)).unwrap();
    rec.write("manual", "ScanResult", &manual);
    let result = manual.result();
    let paths = [
        ".config/nvim",
        ".config/nvim/init.lua",
        ".ssh/id_ed25519",
        ".tmux.conf",
    ]
    .map(|rel| sender.home.join(rel))
    .into_iter()
    .chain([sender.root().join("laptop.carrova")])
    .collect();
    let picked = block_on(send::pick(
        &sender.state,
        &result.session,
        result.selection.clone(),
        paths,
    ))
    .unwrap();
    rec.write("picked", "Picked", &picked);
}

/// Edits beyond ticking a sensitive item, for the fake port's contract
/// test: an app's row, a path, the caches and an unmovable override.
fn more_edits() -> Vec<SelectionEdit> {
    vec![
        SelectionEdit::App {
            app: "git".into(),
            included: false,
        },
        SelectionEdit::Path {
            item: "neovim/config".into(),
            path: "user_config:nvim/lua".into(),
            included: false,
        },
        SelectionEdit::Path {
            item: "neovim/config".into(),
            path: "user_config:nvim/after".into(),
            included: false,
        },
        SelectionEdit::Caches { included: true },
        SelectionEdit::ConfirmSensitive {
            item: "ssh/keys".into(),
            confirmed: true,
        },
        SelectionEdit::Item {
            item: "ssh/keys".into(),
            included: true,
        },
    ]
}

/// Options the review screens set, each on top of `confirmed`, for the
/// fake planner's contract test: the same options re-planned by the fake
/// must give the same plan shape.
fn plan_variants(confirmed: &PlanOptions) -> Vec<PlanOptions> {
    let clash = ItemKey::from("git/config");
    vec![
        PlanOptions::default(),
        PlanOptions {
            default_rule: ConflictRule::KeepBoth,
            ..confirmed.clone()
        },
        PlanOptions {
            item_rules: [(clash.clone(), ConflictRule::Skip)].into(),
            ..confirmed.clone()
        },
        PlanOptions {
            item_rules: [(clash, ConflictRule::Skip)].into(),
            path_rules: [("home:.gitconfig".to_owned(), ConflictRule::KeepBoth)].into(),
            ..confirmed.clone()
        },
        PlanOptions {
            remaps: [(
                ItemKey::from("ssh/config"),
                Remap {
                    root: Some(SemanticRoot::Documents),
                    rel_prefix: Some("old-laptop".into()),
                },
            )]
            .into(),
            ..confirmed.clone()
        },
        PlanOptions {
            items: Some(vec!["ssh/keys".into(), "tmux/config".into()]),
            ..PlanOptions::default()
        },
    ]
}

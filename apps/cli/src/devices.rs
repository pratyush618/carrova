//! `carrova devices list | rename | wake | forget | reset` and `carrova
//! visibility`: the known-device store and who may find this computer,
//! the same values the desktop app shows (state dir).

use std::io::Write;
use std::process::ExitCode;

use anyhow::Result;
use carrova_core::devices::KnownDevices;
use carrova_core::presence::{Visibility, load_visibility, save_visibility};
use carrova_core::time::now_unix;
use carrova_crypto::identity::Fingerprint;
use carrova_platform::Platform;
use carrova_view::devices::{
    VisibilityChoice, chosen_visibility, known_device_views, visibility_view,
};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::json;

use crate::output::{OutputFormat, print_json};
use crate::prompt::{confirm, ensure_can_confirm};
use crate::recipes::seams::platform;
use crate::render::devices as render;

/// `carrova devices`
#[derive(Debug, Args)]
pub struct DevicesArgs {
    #[command(subcommand)]
    command: Option<DevicesCommand>,
    /// Output JSON on stdout.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum DevicesCommand {
    /// List the devices this computer remembers (default).
    List,
    /// Give a known device a new name.
    Rename {
        /// Its fingerprint (`7F3A·19C2`, separator optional).
        device: String,
        /// The new name.
        name: String,
    },
    /// Wake a sleeping known device with a Wake-on-LAN packet (needs its
    /// saved wake hint). Best effort: it may not wake.
    Wake {
        /// Its fingerprint (`7F3A·19C2`, separator optional).
        device: String,
    },
    /// Forget one known device, or all of them. It then needs a code
    /// again.
    Forget {
        /// Its fingerprint (`7F3A·19C2`, separator optional).
        #[arg(required_unless_present = "all", conflicts_with = "all")]
        device: Option<String>,
        /// Forget every known device. Irreversible.
        #[arg(long)]
        all: bool,
        /// Don't ask for confirmation.
        #[arg(short, long)]
        yes: bool,
    },
    /// Forget every device and give this computer a new identity.
    /// Irreversible.
    Reset {
        /// Don't ask for confirmation.
        #[arg(short, long)]
        yes: bool,
    },
}

/// `carrova visibility`
#[derive(Debug, Args)]
pub struct VisibilityArgs {
    /// Who may find this computer nearby (`everyone` lasts 10 minutes);
    /// omit to show the current choice.
    choice: Option<VisibilityArg>,
    /// Output JSON on stdout.
    #[arg(long)]
    json: bool,
}

/// The choices `carrova visibility` can set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum VisibilityArg {
    /// Nobody.
    NoOne,
    /// Known devices only.
    Known,
    /// Everyone, for 10 minutes; then known devices only.
    Everyone,
}

impl From<VisibilityArg> for VisibilityChoice {
    fn from(arg: VisibilityArg) -> Self {
        match arg {
            VisibilityArg::NoOne => Self::NoOne,
            VisibilityArg::Known => Self::Known,
            VisibilityArg::Everyone => Self::Everyone,
        }
    }
}

/// `carrova devices`
pub fn devices(out: &mut impl Write, a: DevicesArgs) -> Result<ExitCode> {
    let format = OutputFormat::from_json_flag(a.json);
    let platform = platform()?;
    let platform = platform.as_ref();
    let store = KnownDevices::open(platform)?;
    match a.command.unwrap_or(DevicesCommand::List) {
        DevicesCommand::List => {
            let list = known_device_views(&store.list(platform)?);
            match format {
                OutputFormat::Json => print_json(out, &list)?,
                OutputFormat::Human => render::device_list(out, &list)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        DevicesCommand::Rename { device, name } => {
            let id = store.resolve_id(platform, &device)?;
            store.rename(platform, &id, &name)?;
            let fingerprint = Fingerprint::of(&id).to_string();
            finish(out, format, &json!({ "renamed": fingerprint }), "Renamed.")
        }
        DevicesCommand::Wake { device } => {
            let id = store.resolve_id(platform, &device)?;
            store.wake(platform, &id)?;
            let fingerprint = Fingerprint::of(&id).to_string();
            let text = "Wake sent. It can take a minute to show up.";
            finish(out, format, &json!({ "woke": fingerprint }), text)
        }
        DevicesCommand::Forget {
            device: Some(device),
            ..
        } => {
            let id = store.resolve_id(platform, &device)?;
            store.forget(platform, &id)?;
            let fingerprint = Fingerprint::of(&id).to_string();
            let text = "Forgotten. It needs a code next time.";
            finish(out, format, &json!({ "forgot": fingerprint }), text)
        }
        DevicesCommand::Forget {
            device: None, yes, ..
        } => {
            let question = "Forget every known device? They need a code next time.";
            if !confirmed(yes, "forgetting every device", question)? {
                return Ok(cancelled());
            }
            store.forget_all(platform)?;
            let text = "Forgot every known device.";
            finish(out, format, &json!({ "forgot": "all" }), text)
        }
        DevicesCommand::Reset { yes } => {
            let question = "Forget every known device and give this computer a new identity?";
            if !confirmed(yes, "resetting this computer's identity", question)? {
                return Ok(cancelled());
            }
            store.reset_identity(platform)?;
            let text = "Forgot every known device. This computer has a new identity.";
            finish(out, format, &json!({ "reset": true }), text)
        }
    }
}

/// The user said no: nothing changed.
fn cancelled() -> ExitCode {
    eprintln!("cancelled; nothing changed");
    ExitCode::FAILURE
}

/// Ask before an irreversible change (`--yes` agrees up front).
fn confirmed(yes: bool, what: &str, question: &str) -> Result<bool> {
    ensure_can_confirm(yes, what)?;
    confirm(yes, &format!("{question} This cannot be undone."))
}

/// A change's result: `json` or the `text` line.
fn finish(
    out: &mut impl Write,
    format: OutputFormat,
    json: &serde_json::Value,
    text: &str,
) -> Result<ExitCode> {
    match format {
        OutputFormat::Json => print_json(out, json)?,
        OutputFormat::Human => writeln!(out, "{text}")?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `carrova visibility [no-one|known|everyone]`
pub fn visibility(out: &mut impl Write, a: &VisibilityArgs) -> Result<ExitCode> {
    let platform = platform()?;
    let visibility = current_or_set(platform.as_ref(), a.choice)?;
    let view = visibility_view(visibility);
    match OutputFormat::from_json_flag(a.json) {
        OutputFormat::Json => print_json(out, &view)?,
        OutputFormat::Human => render::visibility(out, view)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// The saved visibility in force, after saving `choice` if given.
fn current_or_set(platform: &dyn Platform, choice: Option<VisibilityArg>) -> Result<Visibility> {
    let now = now_unix();
    Ok(match choice {
        Some(choice) => {
            let visibility = chosen_visibility(choice.into(), now);
            save_visibility(platform, visibility)?;
            visibility
        }
        None => load_visibility(platform, now)?,
    })
}

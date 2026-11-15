//! `carrova link | clip | file | handoff` and the linked-device parts of
//! `carrova devices`: everything goes through the user's one link
//! supervisor (the app's or `carrova link serve`'s), started for the
//! command's own duration when none runs.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::{
    HandoffItem, PeerState, SupervisorHandle, attach_or_start, link_with_code,
};
use carrova_core::{Cancel, Engine, Ticket};
use carrova_crypto::identity::DeviceId;
use carrova_link::{Feature, FeatureState};
use carrova_platform::Platform;
use carrova_view::ecosystem::{LinkedDeviceView, feature_rows, linked_device_views};
use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::json;

use crate::devices::finish;
use crate::events::CliSink;
use crate::interrupt;
use crate::output::{OutputFormat, print_json};
use crate::recipes::seams;
use crate::render::link as render;
use crate::render::text::safe;

/// How long a command that started the supervisor itself waits for the
/// device to link (presence browse, dial, policy exchange) before trying.
const LINK_WAIT: Duration = Duration::from_secs(10);
/// How often `carrova link serve` checks for Ctrl-C.
const SERVE_POLL: Duration = Duration::from_millis(200);

/// `carrova link`
#[derive(Debug, Args)]
pub struct LinkArgs {
    #[command(subcommand)]
    command: LinkCommand,
    /// Output JSON on stdout.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum LinkCommand {
    /// Show each known device's link and features.
    Status {
        /// Also measure the round trip to each linked device.
        #[arg(long)]
        ping: bool,
    },
    /// Run the link supervisor in the foreground until Ctrl-C (for
    /// computers without the Carrova app running).
    Serve,
}

/// `carrova clip`
#[derive(Debug, Args)]
pub struct ClipArgs {
    #[command(subcommand)]
    command: ClipCommand,
    /// Output JSON on stdout.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum ClipCommand {
    /// Send the current clipboard to a linked device.
    Send {
        /// The device's fingerprint (`7F3A·19C2`, separator optional).
        #[arg(long, value_name = "DEVICE")]
        to: String,
    },
}

/// `carrova file`
#[derive(Debug, Args)]
pub struct FileArgs {
    #[command(subcommand)]
    command: FileCommand,
    /// Output JSON on stdout.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum FileCommand {
    /// Offer a file to a linked device; its user accepts it.
    Send {
        /// The file.
        path: PathBuf,
        /// The device's fingerprint (`7F3A·19C2`, separator optional).
        #[arg(long, value_name = "DEVICE")]
        to: String,
    },
}

/// `carrova handoff`
#[derive(Debug, Args)]
pub struct HandoffArgs {
    /// An `http(s)://` link, or a file, to open on the device.
    target: String,
    /// The device's fingerprint (`7F3A·19C2`, separator optional).
    #[arg(long, value_name = "DEVICE")]
    to: String,
    /// Output JSON on stdout.
    #[arg(long)]
    json: bool,
}

/// One `<feature>=<on|off|ask>` of `carrova devices features`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureSetting {
    feature: Feature,
    state: FeatureState,
}

impl FromStr for FeatureSetting {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let expected = || "expected <feature>=<on|off|ask>".to_owned();
        let (feature, state) = s.split_once('=').ok_or_else(expected)?;
        let feature = serde_json::from_value(feature.into()).map_err(|_| expected())?;
        let state = match state {
            "on" => FeatureState::On,
            "off" => FeatureState::Off,
            "ask" => FeatureState::Ask,
            _ => return Err(expected()),
        };
        Ok(Self { feature, state })
    }
}

/// A device in `carrova link status`: its view, and its round trip with
/// `--ping`.
#[derive(Debug, Serialize)]
pub struct StatusRow {
    /// The device and its features.
    #[serde(flatten)]
    pub device: LinkedDeviceView,
    /// Milliseconds to the device and back (`--ping`, linked devices only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping_ms: Option<u128>,
}

/// `carrova link status | serve`
pub fn link(out: &mut impl Write, a: &LinkArgs) -> Result<ExitCode> {
    let format = OutputFormat::from_json_flag(a.json);
    match a.command {
        LinkCommand::Status { ping } => status(out, format, ping),
        LinkCommand::Serve => serve(out, format),
    }
}

/// `carrova clip send --to <device>`
pub fn clip(out: &mut impl Write, a: ClipArgs) -> Result<ExitCode> {
    let format = OutputFormat::from_json_flag(a.json);
    let ClipCommand::Send { to } = a.command;
    let (handle, device) = linked(&to, format)?;
    handle.send_clipboard(&device.public)?;
    sent(out, format, "clipboard", &device)
}

/// `carrova file send <path> --to <device>`
pub fn file(out: &mut impl Write, a: FileArgs) -> Result<ExitCode> {
    let format = OutputFormat::from_json_flag(a.json);
    let FileCommand::Send { path, to } = a.command;
    let path = absolute(&path)?;
    let (handle, device) = linked(&to, format)?;
    handle.send_file(&device.public, &path)?;
    sent(out, format, "file", &device)
}

/// `carrova handoff <url|path> --to <device>`
pub fn handoff(out: &mut impl Write, a: &HandoffArgs) -> Result<ExitCode> {
    let format = OutputFormat::from_json_flag(a.json);
    // Anything with a scheme is a link; the supervisor refuses non-http(s).
    let (item, what) = if a.target.contains("://") {
        (HandoffItem::Url(a.target.clone()), "link")
    } else {
        (HandoffItem::File(absolute(Path::new(&a.target))?), "file")
    };
    let (handle, device) = linked(&a.to, format)?;
    handle.handoff(&device.public, item)?;
    sent(out, format, what, &device)
}

/// `carrova devices link <code|ticket>`: pair only with the computer
/// showing the code; nothing moves.
pub fn pair(out: &mut impl Write, format: OutputFormat, target: &str) -> Result<ExitCode> {
    let ticket: Ticket = target.parse()?;
    let engine = engine(format)?;
    let cancel = Cancel::default();
    let record = {
        let _armed = interrupt::arm(&cancel, format);
        link_with_code(&engine, &ticket, &cancel)?
    };
    let fingerprint = record.fingerprint().to_string();
    let text = format!("Linked with {} ({fingerprint}).", safe(&record.name));
    let result = json!({ "linked": fingerprint, "name": record.name });
    finish(out, format, &result, &text)
}

/// `carrova devices features <device> [<feature>=<state> ...]`: apply
/// `settings` in order, then show the device's features.
pub fn features(
    out: &mut impl Write,
    format: OutputFormat,
    device: &DeviceId,
    settings: &[FeatureSetting],
) -> Result<ExitCode> {
    let handle = attach_or_start(engine(format)?)?;
    for s in settings {
        handle.set_feature(device, s.feature, s.state)?;
    }
    let peer = handle
        .status()?
        .into_iter()
        .find(|p| p.id == *device)
        .context("the link supervisor does not know this device yet; try again")?;
    let rows = feature_rows(&peer.local, peer.remote.as_ref());
    match format {
        OutputFormat::Json => print_json(out, &rows)?,
        OutputFormat::Human => render::features(out, &rows)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `carrova link status [--ping]`
fn status(out: &mut impl Write, format: OutputFormat, ping: bool) -> Result<ExitCode> {
    let handle = attach_or_start(engine(format)?)?;
    let peers = handle.status()?;
    let rows = peers
        .iter()
        .zip(linked_device_views(&peers))
        .map(|(peer, device)| {
            let ping_ms = if ping && peer.state == PeerState::Linked {
                Some(handle.ping(&peer.id)?.as_millis())
            } else {
                None
            };
            Ok(StatusRow { device, ping_ms })
        })
        .collect::<Result<Vec<_>>>()?;
    match format {
        OutputFormat::Json => print_json(out, &rows)?,
        OutputFormat::Human => render::status(out, &rows)?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `carrova link serve`: host the supervisor until Ctrl-C.
fn serve(out: &mut impl Write, format: OutputFormat) -> Result<ExitCode> {
    let handle = attach_or_start(engine(format)?)?;
    if matches!(handle, SupervisorHandle::Remote(_)) {
        bail!(
            "linked devices are already served (by the Carrova app or another `carrova link serve`)"
        );
    }
    if format == OutputFormat::Human {
        writeln!(out, "Serving linked devices. Press Ctrl-C to stop.")?;
        out.flush()?;
    }
    let cancel = Cancel::default();
    let _armed = interrupt::arm(&cancel, format);
    while !cancel.is_cancelled() {
        thread::sleep(SERVE_POLL);
    }
    drop(handle);
    finish(out, format, &json!({ "served": true }), "Stopped.")
}

/// The known device `to` names and the supervisor, after giving a
/// supervisor started here [`LINK_WAIT`] to link it.
fn linked(to: &str, format: OutputFormat) -> Result<(SupervisorHandle, DeviceRecord)> {
    let engine = engine(format)?;
    let platform = &**engine.platform();
    let device = KnownDevices::open(platform)?.resolve(platform, to)?;
    let handle = attach_or_start(engine)?;
    if matches!(handle, SupervisorHandle::Local(_)) {
        wait_ready(&handle, &device.public)?;
    }
    Ok((handle, device))
}

/// Wait until `device` is linked with its policy known, at most
/// [`LINK_WAIT`]; the caller's request then reports why it is not.
fn wait_ready(handle: &SupervisorHandle, device: &DeviceId) -> Result<()> {
    let events = handle.subscribe()?;
    let deadline = Instant::now() + LINK_WAIT;
    let ready = || -> Result<bool> {
        Ok(handle
            .status()?
            .iter()
            .any(|p| p.id == *device && p.state == PeerState::Linked && p.remote.is_some()))
    };
    while !ready()? {
        let left = deadline.saturating_duration_since(Instant::now());
        // Any event may be the link or its policy: check again.
        if left.is_zero() || events.recv_timeout(left).is_err() {
            break;
        }
    }
    Ok(())
}

/// `path` made absolute, as the supervisor requires.
fn absolute(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).with_context(|| format!("cannot open {}", path.display()))
}

/// The result of a send to `device`.
fn sent(
    out: &mut impl Write,
    format: OutputFormat,
    what: &str,
    device: &DeviceRecord,
) -> Result<ExitCode> {
    let fingerprint = device.fingerprint().to_string();
    let text = format!("Sent the {what} to {}.", safe(&device.name));
    finish(
        out,
        format,
        &json!({ "sent": what, "to": fingerprint }),
        &text,
    )
}

/// An engine for linked devices (no recipes) on the current platform.
/// Events of a supervisor hosted here print as JSON lines with `--json`;
/// the human renderer prints no link events (verbosity only affects
/// session events, which link commands never emit).
fn engine(format: OutputFormat) -> Result<Engine> {
    let platform: Arc<dyn Platform> = Arc::from(seams::platform()?);
    let sink = Arc::new(CliSink::new(format, 0));
    Ok(Engine::new(platform, Vec::new(), sink))
}

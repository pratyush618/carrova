//! Test fixtures: one machine (fake platform over a temp home and state
//! dir, a one-recipe tree) with the bridge state on it, recording events.
#![allow(clippy::unwrap_used)]

mod recorder;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use carrova_bundle::RecipeLocations;
use carrova_core::KdfParams;
use carrova_platform::fake::FakePlatform;
use carrova_view::ipc::BridgeEvent;
use tempfile::TempDir;

use crate::events::Emit;
use crate::state::{AppState, BridgeConfig};

pub(crate) use recorder::Recorder;

/// The test app: a config item (suggested) and SSH keys (sensitive).
const RECIPE: &str = "\
schema: 2
id: app
name: The App
category: test
description: A test app.
detect:
  linux:
    - binary: app
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: app }
    flags:
      default: true
  - id: keys
    label: Keys
    paths:
      - { root: home, rel: .ssh }
    flags:
      sensitive: true
      default: false
";

/// Free space of every fake filesystem (120 GB).
const FREE_SPACE: u64 = 120_000_000_000;

/// Archive passphrase used by tests.
pub(crate) const PASS: &str = "correct horse battery staple";

/// Called for every event, after it is recorded.
type Hook = Box<dyn Fn(&serde_json::Value) + Send + Sync>;

/// A [`Recorder`] that can also react to events (e.g. cancel on one).
#[derive(Default)]
pub(crate) struct Events {
    pub(crate) recorded: Recorder,
    hook: Mutex<Option<Hook>>,
}

impl Events {
    /// Call `hook` for every later event.
    pub(crate) fn on_event(&self, hook: impl Fn(&serde_json::Value) + Send + Sync + 'static) {
        *self.hook.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(hook));
    }
}

impl Emit for Events {
    fn emit(&self, event: &BridgeEvent) {
        self.recorded.emit(event);
        let json = serde_json::to_value(event).unwrap_or_default();
        if let Some(hook) = &*self.hook.lock().unwrap_or_else(PoisonError::into_inner) {
            hook(&json);
        }
    }
}

/// One machine with the bridge on it.
pub(crate) struct Machine {
    tmp: TempDir,
    pub(crate) home: PathBuf,
    pub(crate) events: Arc<Events>,
    pub(crate) state: AppState,
}

impl Machine {
    /// A machine with the one-recipe test tree and the `app` binary.
    pub(crate) fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let tree = tmp.path().join("recipes");
        std::fs::create_dir_all(tree.join("test/app")).unwrap();
        std::fs::write(tree.join("test/app/recipe.yaml"), RECIPE).unwrap();
        Self::with_recipes(tmp, tree, &["app"])
    }

    /// A machine in `tmp` using the recipe tree `tree`, with `binaries`
    /// installed (for detection).
    pub(crate) fn with_recipes(tmp: TempDir, tree: PathBuf, binaries: &[&str]) -> Self {
        let home = tmp.path().join("home");
        std::fs::create_dir_all(home.join(".config")).unwrap();
        let platform = binaries.iter().fold(
            FakePlatform::with_home(&home)
                .with_state_dir(tmp.path().join("state/carrova"))
                .free_space_bytes(FREE_SPACE),
            |platform, &bin| platform.binary(bin, format!("/usr/bin/{bin}")),
        );
        let events = Arc::new(Events::default());
        let state = AppState::new(
            Arc::new(platform),
            RecipeLocations {
                dev_override: Some(tree),
                ..RecipeLocations::default()
            },
            events.clone(),
            BridgeConfig {
                kdf: |_| KdfParams::FLOOR,
                answer_timeout: Duration::from_millis(50),
            },
        );
        Self {
            tmp,
            home,
            events,
            state,
        }
    }

    /// The temp dir everything of this machine lives in.
    pub(crate) fn root(&self) -> &std::path::Path {
        self.tmp.path()
    }

    /// Write `bytes` to `rel` under the home directory.
    pub(crate) fn put(&self, rel: &str, bytes: &[u8]) {
        let path = self.home.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, bytes).unwrap();
    }

    /// A path in this machine's scratch space (outside home).
    pub(crate) fn scratch(&self, name: &str) -> PathBuf {
        self.tmp.path().join(name)
    }
}

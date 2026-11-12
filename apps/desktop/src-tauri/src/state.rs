//! [`AppState`]: everything the commands share, managed by Tauri. Cheap
//! to clone (every part is shared), so a command can move a copy into a
//! worker.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use carrova_bundle::RecipeLocations;
use carrova_core::time::now_unix;
use carrova_core::{Engine, Inbox, KdfParams, KdfStrength, NoticeKind, SessionId};
use carrova_platform::Platform;

use crate::command_error::CommandError;
use crate::events::{Emit, SessionSink};
use crate::pairing::{ANSWER_TIMEOUT, PairingAnswers};
use crate::recipes::RecipeService;
use crate::registry::Registry;
use crate::send_request::SendRequests;
use crate::settings::SettingsStore;

/// How the bridge runs, beyond where it runs.
#[derive(Debug, Clone, Copy)]
pub struct BridgeConfig {
    /// Argon2id parameters of each archive strength the user can pick:
    /// [`KdfStrength::params`] in the app (the engine's one mapping);
    /// tests inject cheaper parameters.
    pub kdf: fn(KdfStrength) -> KdfParams,
    /// How long a "show a new code?" question waits.
    pub answer_timeout: Duration,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            kdf: KdfStrength::params,
            answer_timeout: ANSWER_TIMEOUT,
        }
    }
}

/// Shared state of the bridge.
#[derive(Clone)]
pub struct AppState {
    /// The machine Carrova runs on.
    pub platform: Arc<dyn Platform>,
    /// Recipes for new sessions.
    pub recipes: Arc<RecipeService>,
    /// Live sessions.
    pub sessions: Arc<Registry>,
    /// Open pairing questions.
    pub answers: Arc<PairingAnswers>,
    /// User settings.
    pub settings: SettingsStore,
    /// The bell's notices.
    pub inbox: Inbox,
    /// Paths sent from outside, until the UI takes them.
    pub pending_send: SendRequests,
    /// Held across every check-and-write of the file-manager entries, so
    /// the start-up refresh never undoes the user's latest choice.
    pub entries_lock: Arc<Mutex<()>>,
    /// Where events go.
    pub out: Arc<dyn Emit>,
    /// Run options.
    pub config: BridgeConfig,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("platform", &self.platform)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AppState {
    /// The bridge on `platform`, with recipes from `locations`, emitting on
    /// `out`.
    ///
    /// # Errors
    ///
    /// The notification inbox cannot be read.
    pub fn new(
        platform: Arc<dyn Platform>,
        locations: RecipeLocations,
        out: Arc<dyn Emit>,
        config: BridgeConfig,
    ) -> Result<Self, CommandError> {
        let (inbox, set_aside) = Inbox::open(Arc::clone(&platform))?;
        if set_aside.is_some()
            && let Err(error) = inbox.push(
                NoticeKind::MoveFailed,
                now_unix(),
                None,
                "The notification list was damaged. It was set aside and started again.".into(),
            )
        {
            tracing::warn!(%error, "could not tell the user the inbox was set aside");
        }
        Ok(Self {
            inbox,
            recipes: Arc::new(RecipeService::new(Arc::clone(&platform), locations)),
            sessions: Arc::new(Registry::default()),
            answers: Arc::new(PairingAnswers::default()),
            settings: SettingsStore::new(Arc::clone(&platform)),
            pending_send: SendRequests::default(),
            entries_lock: Arc::default(),
            platform,
            out,
            config,
        })
    }

    /// An engine reporting to `sink`, with the current recipes and
    /// plugins, remembering devices as the settings say. Blocking
    /// (recipes resolve on first use).
    ///
    /// # Errors
    ///
    /// Recipes cannot be resolved.
    pub fn engine(&self, sink: Arc<SessionSink>) -> Result<Engine, CommandError> {
        let recipes = self.recipes.current()?;
        let remember = self.settings.remember_devices();
        Ok(Engine::new(
            Arc::clone(&self.platform),
            Arc::clone(&recipes.recipes),
            sink,
        )
        .with_plugins(Arc::clone(&recipes.plugins))
        .with_remember_devices(remember))
    }

    /// A sink for a session that does not exist yet.
    pub fn unbound_sink(&self) -> Arc<SessionSink> {
        Arc::new(SessionSink::unbound(Arc::clone(&self.out)))
    }

    /// A sink for the existing session `id`.
    pub fn sink_for(&self, id: &SessionId) -> Arc<SessionSink> {
        Arc::new(SessionSink::bound(Arc::clone(&self.out), id))
    }
}

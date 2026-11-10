//! [`DANGEROUS_PATHS`]: receiver locations where incoming data can run
//! code, grant access or change how the user logs in.

use carrova_platform::SemanticRoot;

/// A receiver location where incoming data can run code, grant access or
/// change how the user logs in. Matches the path itself and everything
/// below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DangerousPath {
    pub root: SemanticRoot,
    pub rel: &'static str,
    /// Why it is dangerous, shown to the user.
    pub why: &'static str,
}

const fn dp(root: SemanticRoot, rel: &'static str, why: &'static str) -> DangerousPath {
    DangerousPath { root, rel, why }
}

const LOGIN: &str = "can run code at login";
const LOGIN_ENV: &str = "changes the login environment";
const GIT: &str = "can make git run commands (core.sshCommand, hooks)";
const SHELL_START: &str = "zsh runs this plugin/theme code at every shell start";
const COMPLETION: &str = "bash runs these completion scripts in every interactive shell";

/// Built-in dangerous locations. Any item with a final target (after remap)
/// equal to or under one of these — or whose handler changes such a file —
/// is sensitive. Matched on `(root, rel)` and on the resolved absolute path
/// (so `home:.config/autostart` hits `user_config:autostart`); `user_config`
/// / `user_data` entries also match their default `home:.config/…` /
/// `home:.local/share/…` spelling even when XDG dirs are moved.
pub const DANGEROUS_PATHS: &[DangerousPath] = &[
    dp(
        SemanticRoot::Home,
        ".ssh",
        "controls SSH logins, keys and commands",
    ),
    dp(
        SemanticRoot::Home,
        ".gnupg",
        "holds GnuPG keys, trust and agent programs",
    ),
    dp(SemanticRoot::Home, ".bashrc", LOGIN),
    dp(SemanticRoot::Home, ".bash_profile", LOGIN),
    dp(SemanticRoot::Home, ".bash_login", LOGIN),
    dp(SemanticRoot::Home, ".bash_logout", LOGIN),
    dp(SemanticRoot::Home, ".bash_aliases", LOGIN),
    dp(SemanticRoot::Home, ".profile", LOGIN),
    dp(SemanticRoot::Home, ".zshrc", LOGIN),
    dp(SemanticRoot::Home, ".zshenv", LOGIN),
    dp(SemanticRoot::Home, ".zprofile", LOGIN),
    dp(SemanticRoot::Home, ".zlogin", LOGIN),
    dp(SemanticRoot::Home, ".zlogout", LOGIN),
    dp(SemanticRoot::UserConfig, "zsh", LOGIN),
    dp(SemanticRoot::UserConfig, "fish", LOGIN),
    dp(SemanticRoot::Home, ".xprofile", LOGIN),
    dp(SemanticRoot::Home, ".xsessionrc", LOGIN),
    dp(SemanticRoot::Home, ".xinitrc", LOGIN),
    // Run by Xsession as the session itself when present.
    dp(SemanticRoot::Home, ".xsession", LOGIN),
    // Shell frameworks: sourced from .zshrc at every shell start; plugins
    // often carry their own `.git/hooks` executables.
    dp(SemanticRoot::Home, ".oh-my-zsh", SHELL_START),
    dp(SemanticRoot::Home, ".zprezto", SHELL_START),
    // bash-completion sources ~/.bash_completion at shell start and the
    // per-command scripts under user_data on first completion.
    dp(SemanticRoot::Home, ".bash_completion", COMPLETION),
    dp(SemanticRoot::UserData, "bash-completion", COMPLETION),
    dp(
        SemanticRoot::UserConfig,
        "autostart",
        "starts programs at login (autostart)",
    ),
    dp(
        SemanticRoot::UserConfig,
        "systemd/user",
        "starts services at login (systemd user units)",
    ),
    dp(
        SemanticRoot::UserData,
        "systemd/user",
        "starts services at login (systemd user units)",
    ),
    dp(SemanticRoot::UserConfig, "environment.d", LOGIN_ENV),
    dp(SemanticRoot::Home, ".pam_environment", LOGIN_ENV),
    dp(
        SemanticRoot::Home,
        ".local/bin",
        "puts programs on the command search path",
    ),
    dp(SemanticRoot::Home, ".gitconfig", GIT),
    dp(SemanticRoot::UserConfig, "git", GIT),
];

impl DangerousPath {
    /// The default `home:` spelling of a `user_config` / `user_data`
    /// location (`.config/…` / `.local/share/…`), matched even when the
    /// XDG dirs are moved.
    #[must_use]
    pub fn home_alias(&self) -> Option<String> {
        match self.root {
            SemanticRoot::UserConfig => Some(format!(".config/{}", self.rel)),
            SemanticRoot::UserData => Some(format!(".local/share/{}", self.rel)),
            _ => None,
        }
    }
}

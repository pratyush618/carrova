// Recorded by `just ipc-fixtures`; do not edit.
import type { CatalogEntry } from '../../bindings/CatalogEntry';

export const catalog: CatalogEntry[] = [
  {
    category: 'shells',
    description:
      'Bash startup files, aliases and readline config. History is a separate, sensitive item (it can contain secrets typed on the command line).',
    detected: true,
    id: 'bash',
    items: [
      {
        default: true,
        id: 'config',
        label: 'Bash startup files, aliases and readline config',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'history',
        label: 'Bash command history',
        sensitive: true,
        unmovable: false,
      },
    ],
    name: 'Bash',
    tier: 'unverified',
  },
  {
    category: 'dev-tools',
    description:
      'Cargo global config (~/.cargo/config.toml). Registry credentials are a separate, sensitive item. The downloaded crate cache is large and off by default; installed binaries are reinstalled, not copied.',
    detected: false,
    id: 'cargo',
    items: [
      {
        default: true,
        id: 'config',
        label: 'Cargo global config (config.toml)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'credentials',
        label: 'Registry credentials (credentials.toml; API tokens)',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'crate-cache',
        label: 'Downloaded crates and git dependencies (cache, large, re-downloaded on demand)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'installed-binaries',
        label: 'Binaries installed with cargo install',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'Cargo',
    tier: 'unverified',
  },
  {
    category: 'dev-tools',
    description:
      'Docker CLI contexts and client config. config.json can hold registry logins, so it is sensitive and off by default; logins kept by a credential helper live in the system keyring and cannot be moved. Images, containers and volumes are not user config and are never captured.',
    detected: false,
    id: 'docker',
    items: [
      {
        default: true,
        id: 'contexts',
        label: 'Docker contexts (remote and alternative engine endpoints)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'context-tls',
        label: 'TLS client certificates and keys for Docker contexts',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'config',
        label: 'Docker client config (config.json; may contain registry logins)',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'cli-plugins',
        label: 'CLI plugins such as Compose and Buildx (reinstallable)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'credential-helper-logins',
        label: 'Registry logins stored by a credential helper (credsStore/credHelpers)',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'Docker CLI',
    tier: 'unverified',
  },
  {
    category: 'shells',
    description:
      'fish config, functions, conf.d snippets and completions, including plugins installed by Fisher (they live in the same folders). Universal variables are a separate, sensitive item (they can hold exported tokens); command history is a separate, sensitive item.',
    detected: false,
    id: 'fish',
    items: [
      {
        default: true,
        id: 'config',
        label: 'fish config, functions, conf.d and completions',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'universal-variables',
        label:
          'Universal variables (abbreviations, colours, fish_user_paths and anything set with set -U, which can include exported tokens or API keys)',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'history',
        label: 'fish command history',
        sensitive: true,
        unmovable: false,
      },
    ],
    name: 'fish',
    tier: 'unverified',
  },
  {
    category: 'dev-tools',
    description:
      'GitHub CLI settings and aliases (config.yml). Your login (hosts.yml) can hold OAuth tokens and is a separate, sensitive item. Tokens kept in the system keyring cannot be moved; run gh auth login on the new machine.',
    detected: false,
    id: 'gh',
    items: [
      {
        default: true,
        id: 'config',
        label: 'GitHub CLI settings and aliases (config.yml)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'hosts',
        label: 'GitHub CLI login (hosts.yml; may contain OAuth tokens)',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'extensions',
        label: 'Installed gh extensions (reinstallable)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'keyring-token',
        label: 'Login tokens stored in the system keyring',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'GitHub CLI',
    tier: 'unverified',
  },
  {
    category: 'dev-tools',
    description:
      'Global Git configuration (user identity, aliases, global ignore and attributes). Stored credentials are a separate, sensitive item.',
    detected: true,
    id: 'git',
    items: [
      {
        default: true,
        id: 'config',
        label: 'Global Git config, ignore and attributes',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'credentials',
        label: 'Stored Git credentials (plain-text tokens/passwords)',
        sensitive: true,
        unmovable: false,
      },
    ],
    name: 'Git',
    tier: 'unverified',
  },
  {
    category: 'security',
    description:
      'GnuPG configuration, public keyring and secret keys. Public keys and ownertrust move through the built-in gpg-keyring handler (`gpg --export` / `--import`, merged into the receiver keyring). Secret keys are copied as raw files (keygrip-named, merge cleanly) with the GnuPG daemons stopped.',
    detected: false,
    id: 'gnupg',
    items: [
      {
        default: true,
        id: 'config',
        label: 'GnuPG, gpg-agent and dirmngr config',
        sensitive: false,
        unmovable: false,
      },
      {
        default: true,
        id: 'public-keyring',
        label: 'Public keyring and trust database',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'secret-keys',
        label: 'Secret keys and revocation certificates',
        sensitive: true,
        unmovable: false,
      },
    ],
    name: 'GnuPG',
    tier: 'unverified',
  },
  {
    category: 'editors',
    description:
      'Neovim configuration (init.lua or init.vim, Lua modules, plugin lockfile). Installed plugins and language servers are reinstallable and off by default. Editing history (ShaDa), swap and undo state are never captured.',
    detected: true,
    id: 'neovim',
    items: [
      {
        default: true,
        id: 'config',
        label: 'Neovim config (init.lua/init.vim, Lua modules, plugin lockfile)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'plugins',
        label: 'Installed plugins and language servers (reinstallable, can be large)',
        sensitive: false,
        unmovable: false,
      },
    ],
    name: 'Neovim',
    tier: 'unverified',
  },
  {
    category: 'dev-tools',
    description:
      'npm user config (~/.npmrc). It often contains registry auth tokens, so it is sensitive and off by default. The package cache and logs are never captured; global packages are reinstalled, not copied.',
    detected: false,
    id: 'npm',
    items: [
      {
        default: false,
        id: 'npmrc',
        label: 'npm user config (.npmrc; may contain registry auth tokens)',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'global-packages',
        label: 'Globally installed packages',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'npm',
    tier: 'unverified',
  },
  {
    category: 'notes',
    description:
      "Obsidian's list of vaults, with home folder paths rewritten for this computer by the path-rewrite plugin. Vaults are ordinary folders and move separately; caches are never captured.",
    detected: false,
    id: 'obsidian',
    items: [
      {
        default: true,
        id: 'vaults',
        label: 'Vault list',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'vault-contents',
        label: 'Vault folders',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'Obsidian',
    tier: 'unverified',
  },
  {
    category: 'shells',
    description:
      'PowerShell 7 (pwsh) profile scripts for the current user. Modules are reinstalled from the PowerShell Gallery, not copied. Command history (PSReadLine) is a separate, sensitive item. Windows PowerShell 5.1 profiles are not included.',
    detected: false,
    id: 'powershell',
    items: [
      {
        default: true,
        id: 'profiles',
        label: 'PowerShell profile scripts (current user, all hosts, VS Code host)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'history',
        label: 'PowerShell command history (PSReadLine)',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'modules',
        label: 'Modules installed for the current user',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'PowerShell',
    tier: 'unverified',
  },
  {
    category: 'security',
    description:
      'OpenSSH client configuration, known hosts, authorized keys and key pairs. Private keys are sensitive and off by default; permissions (600) are restored exactly.',
    detected: true,
    id: 'ssh',
    items: [
      {
        default: true,
        id: 'config',
        label: 'SSH client config and known hosts',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'authorized-keys',
        label: 'Authorized keys (lets those keys log in to this machine)',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'keys',
        label: 'SSH key pairs (private keys)',
        sensitive: true,
        unmovable: false,
      },
    ],
    name: 'OpenSSH',
    tier: 'unverified',
  },
  {
    category: 'shells',
    description:
      'Starship prompt configuration (starship.toml). The line that enables Starship lives in your shell startup file and moves with that shell. Logs in the cache folder are never captured.',
    detected: true,
    id: 'starship',
    items: [
      {
        default: true,
        id: 'config',
        label: 'Starship prompt config (starship.toml)',
        sensitive: false,
        unmovable: false,
      },
    ],
    name: 'Starship',
    tier: 'unverified',
  },
  {
    category: 'editors',
    description:
      'Sublime Text 4 user settings, key bindings, snippets and your Package Control package list (Packages/User). Packages are reinstalled by Package Control from that list. The licence key is a separate, sensitive item; session and cache files are never captured.',
    detected: false,
    id: 'sublime-text',
    items: [
      {
        default: true,
        id: 'settings',
        label: 'Settings, key bindings, snippets and package list (Packages/User)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'license',
        label: 'Licence key',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'packages',
        label: 'Installed packages',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'Sublime Text',
    tier: 'unverified',
  },
  {
    category: 'dev-tools',
    description:
      'tmux configuration (~/.tmux.conf or ~/.config/tmux) and your own scripts. Plugins installed by TPM are reinstallable and off by default. Saved sessions (tmux-resurrect) can contain pane contents and are never captured.',
    detected: true,
    id: 'tmux',
    items: [
      {
        default: true,
        id: 'config',
        label: 'tmux config and scripts',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'plugins',
        label: 'Plugins installed by TPM (reinstallable)',
        sensitive: false,
        unmovable: false,
      },
    ],
    name: 'tmux',
    tier: 'unverified',
  },
  {
    category: 'editors',
    description:
      'Vim and gVim startup files plus your ~/.vim runtime directory (ftplugins, colour schemes, autoload scripts). Plugins installed by a plugin manager are reinstallable and off by default. Swap, undo and backup files are never captured; editing history (viminfo) is a separate, sensitive item.',
    detected: false,
    id: 'vim',
    items: [
      {
        default: true,
        id: 'config',
        label: 'Vim startup files and runtime directory',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'plugins',
        label: 'Plugins installed by a plugin manager (reinstallable)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'history',
        label: 'Vim editing history (viminfo; command line, search and registers)',
        sensitive: true,
        unmovable: false,
      },
    ],
    name: 'Vim',
    tier: 'unverified',
  },
  {
    category: 'editors',
    description:
      'VS Code user settings, keybindings, snippets, tasks and profiles. Caches and per-workspace state are never captured. Extensions are reinstalled, not copied.',
    detected: false,
    id: 'vscode',
    items: [
      {
        default: true,
        id: 'settings',
        label: 'Settings, keybindings, snippets, tasks and profiles',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'global-state',
        label: 'Global UI and extension state (state databases)',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'extensions',
        label: 'Installed extensions',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'Visual Studio Code',
    tier: 'unverified',
  },
  {
    category: 'terminals',
    description:
      'WezTerm Lua configuration (~/.wezterm.lua or the wezterm config folder with its modules). The same config works on Linux, macOS and Windows.',
    detected: false,
    id: 'wezterm',
    items: [
      {
        default: true,
        id: 'config',
        label: 'WezTerm config (wezterm.lua and Lua modules)',
        sensitive: false,
        unmovable: false,
      },
    ],
    name: 'WezTerm',
    tier: 'unverified',
  },
  {
    category: 'terminals',
    description:
      'Windows Terminal settings (profiles, colour schemes, actions and key bindings in settings.json) for the Store, Preview and unpackaged installs. Window layout state is never captured.',
    detected: false,
    id: 'windows-terminal',
    items: [
      {
        default: true,
        id: 'settings',
        label: 'Profiles, colour schemes, actions and key bindings (settings.json)',
        sensitive: false,
        unmovable: false,
      },
    ],
    name: 'Windows Terminal',
    tier: 'unverified',
  },
  {
    category: 'editors',
    description:
      'Zed settings, key bindings, tasks, snippets and themes. Extensions are reinstalled, not copied. Databases and caches are never captured.',
    detected: false,
    id: 'zed',
    items: [
      {
        default: true,
        id: 'config',
        label: 'Settings, key bindings, tasks, snippets and themes',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'extensions',
        label: 'Installed extensions',
        sensitive: false,
        unmovable: true,
      },
    ],
    name: 'Zed',
    tier: 'unverified',
  },
  {
    category: 'shells',
    description:
      'Zsh startup files, Powerlevel10k config and Oh My Zsh customisations. The Oh My Zsh framework itself is reinstalled, not copied. Completion dumps are caches and never captured.',
    detected: false,
    id: 'zsh',
    items: [
      {
        default: true,
        id: 'config',
        label: 'Zsh startup files and prompt config',
        sensitive: false,
        unmovable: false,
      },
      {
        default: true,
        id: 'oh-my-zsh-custom',
        label: 'Oh My Zsh custom plugins, themes and scripts',
        sensitive: false,
        unmovable: false,
      },
      {
        default: false,
        id: 'oh-my-zsh',
        label: 'Oh My Zsh framework',
        sensitive: false,
        unmovable: true,
      },
      {
        default: false,
        id: 'history',
        label: 'Zsh command history',
        sensitive: true,
        unmovable: false,
      },
      {
        default: false,
        id: 'sessions',
        label: 'Zsh per-session history (.zsh_sessions)',
        sensitive: true,
        unmovable: false,
      },
    ],
    name: 'Zsh',
    tier: 'unverified',
  },
];

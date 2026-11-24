import { Shot } from '../Shot';

// Mirrors recipes/<category>/<id>/. The monogram is the id's first letters, as in the app.
const GROUPS: readonly { name: string; apps: readonly (readonly [id: string, name: string])[] }[] =
  [
    {
      name: 'Editors and notes',
      apps: [
        ['vscode', 'VS Code'],
        ['zed', 'Zed'],
        ['neovim', 'Neovim'],
        ['vim', 'Vim'],
        ['sublime-text', 'Sublime Text'],
        ['obsidian', 'Obsidian'],
      ],
    },
    {
      name: 'Shells and terminals',
      apps: [
        ['zsh', 'Zsh'],
        ['bash', 'Bash'],
        ['fish', 'fish'],
        ['powershell', 'PowerShell'],
        ['starship', 'Starship'],
        ['wezterm', 'WezTerm'],
        ['windows-terminal', 'Windows Terminal'],
        ['tmux', 'tmux'],
      ],
    },
    {
      name: 'Developer tools',
      apps: [
        ['git', 'Git'],
        ['gh', 'GitHub CLI'],
        ['docker', 'Docker CLI'],
        ['npm', 'npm'],
        ['cargo', 'Cargo'],
      ],
    },
    {
      name: 'Keys',
      apps: [
        ['ssh', 'OpenSSH'],
        ['gnupg', 'GnuPG'],
      ],
    },
  ];

export function Apps() {
  return (
    <section className="apps" aria-labelledby="apps-title">
      <h2 id="apps-title" className="section-title">
        Knows where your apps keep things.
      </h2>
      <p className="section-sub">
        Each app has a recipe that says what to bring and where it goes. Built-in recipes are
        reviewed and signed, and Carrova checks the signature before using one.
      </p>
      <div className="apps-grid">
        <div className="app-groups">
          {GROUPS.map((g) => (
            <div key={g.name} className="app-group">
              <h3>{g.name}</h3>
              <ul>
                {g.apps.map(([id, name]) => (
                  <li key={id}>
                    <span className="glyph" aria-hidden="true">
                      {id.slice(0, 3)}
                    </span>
                    {name}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
        <div className="window apps-shot">
          <Shot
            name="apps"
            sizes="(min-width: 1360px) 700px, calc(100vw - 32px)"
            alt="The Apps screen: SSH, verified and signed by the Carrova team, with what moves, what's sensitive and what's left out."
          />
        </div>
      </div>
      <p className="fine">
        Missing yours? Pick any files or folders by hand, or{' '}
        <a className="more" href="https://github.com/pratyush618/carrova/tree/main/recipes">
          add a recipe
        </a>
      </p>
    </section>
  );
}

const APPS = [
  'Visual Studio Code',
  'Obsidian',
  'Zed',
  'Neovim',
  'Sublime Text',
  'Vim',
  'Git',
  'GitHub CLI',
  'Docker CLI',
  'npm',
  'Cargo',
  'Zsh',
  'Bash',
  'fish',
  'PowerShell',
  'Starship',
  'tmux',
  'WezTerm',
  'Windows Terminal',
  'OpenSSH',
  'GnuPG',
] as const;

export function Apps() {
  return (
    <section className="apps" aria-labelledby="apps-title">
      <h2 id="apps-title" className="section-title">
        Knows where your apps keep things.
      </h2>
      <ul className="app-list">
        {APPS.map((a) => (
          <li key={a}>{a}</li>
        ))}
      </ul>
      <p className="fine">
        Missing yours? Pick any files or folders by hand, or{' '}
        <a className="more" href="https://github.com/pratyush618/carrova/tree/main/recipes">
          add a recipe
        </a>
      </p>
    </section>
  );
}

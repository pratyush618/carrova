import { ThemeSwitch } from '../ThemeSwitch';

export function Nav() {
  return (
    <header className="nav">
      <div className="nav-inner">
        <a className="brand" href="#top" aria-label="Carrova, back to top">
          <img src="./icon.svg" alt="" width="22" height="22" />
          <span className="brand-name">Carrova</span>
        </a>
        <nav aria-label="Sections">
          <a href="#how">How it works</a>
          <a href="#privacy">Privacy</a>
          <a href="#phone">Phone</a>
          <a href="https://github.com/pratyush618/carrova">GitHub</a>
        </nav>
        <ThemeSwitch />
        <a className="btn btn-sm" href="#download">
          Download
        </a>
      </div>
    </header>
  );
}

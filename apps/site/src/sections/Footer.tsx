export function Footer({ version }: { readonly version: string | undefined }) {
  return (
    <footer className="footer">
      <div className="footer-inner">
        <p className="brand">
          <img src="./icon.svg" alt="" width="20" height="20" />
          Carrova {version && <span className="dim">{version.replace(/^v/, '')}</span>}
        </p>
        <nav aria-label="Project">
          <a href="https://github.com/pratyush618/carrova">Source code</a>
          <a href="https://github.com/pratyush618/carrova/releases">Releases</a>
          <a href="https://github.com/pratyush618/carrova/blob/main/SECURITY.md">Security</a>
          <a href="https://github.com/pratyush618/carrova/issues">Report a problem</a>
        </nav>
        <p className="fine">Apache-2.0. This site sets no cookies and loads no trackers.</p>
      </div>
    </footer>
  );
}

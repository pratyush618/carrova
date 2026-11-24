import { visitorOs } from '../download/DownloadButton';
import { fileUrl, PLATFORMS, RELEASES_URL } from '../download/platform';
import type { Release } from '../download/useRelease';

export function Download({ release }: { readonly release: Release | undefined }) {
  const assets = release?.assets ?? [];
  // The visitor's own system first.
  const ordered = [...PLATFORMS].sort(
    (a, b) => Number(b.os === visitorOs) - Number(a.os === visitorOs),
  );
  return (
    <section className="download" id="download" aria-labelledby="download-title">
      <h2 id="download-title" className="section-title">
        Get Carrova.
      </h2>
      <p className="section-sub">
        Free, on both computers. {release && <span className="mono">{release.version}</span>}
      </p>
      <ul className="platforms">
        {ordered.map((p) => (
          <li key={p.os} className={p.os === visitorOs ? 'is-yours' : undefined}>
            <div className="platform-head">
              <h3>{p.name}</h3>
              {p.beta && <span className="tag">Beta</span>}
              {p.os === visitorOs && <span className="tag tag-you">Your system</span>}
            </div>
            <p className="platform-req">{p.requires}</p>
            <div className="files">
              {p.files.map((f, i) => (
                <a
                  key={f.suffix}
                  className={i === 0 ? 'file file-main' : 'file'}
                  href={fileUrl(assets, f)}
                >
                  {f.label}
                </a>
              ))}
            </div>
            {p.note && <p className="platform-note">{p.note}</p>}
          </li>
        ))}
      </ul>
      <p className="fine">
        Install steps and checksums are on{' '}
        <a className="more" href={RELEASES_URL}>
          the release page
        </a>
      </p>
    </section>
  );
}

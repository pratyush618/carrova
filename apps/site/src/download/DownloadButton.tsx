import { detectOs, fileUrl, PLATFORMS, primaryFile } from './platform';
import { useVisitorArch } from './useVisitorArch';
import type { Release } from './useRelease';

const visitorOs = detectOs(navigator.userAgent, navigator.platform);

/** Primary call to action: the visitor's own OS when we can tell, otherwise the download list. */
export function DownloadButton({ release }: { readonly release: Release | undefined }) {
  const platform = PLATFORMS.find((p) => p.os === visitorOs);
  const arch = useVisitorArch();
  const file = platform && primaryFile(platform, arch);
  if (!platform || !file) {
    return (
      <a className="btn" href="#download">
        Download Carrova
      </a>
    );
  }
  return (
    <a className="btn" href={fileUrl(release?.assets ?? [], file)}>
      Download for {platform.name}
      {platform.beta && <span className="btn-tag">Beta</span>}
    </a>
  );
}

export { visitorOs };

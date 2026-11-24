export type Os = 'linux' | 'windows' | 'mac' | 'android';
export type Arch = 'x64' | 'arm64';

export interface ReleaseAsset {
  readonly name: string;
  readonly browser_download_url: string;
}

export interface DownloadFile {
  readonly label: string;
  /** Asset names end with this, e.g. `Carrova_0.3.1_amd64.deb`. */
  readonly suffix: string;
}

export interface Platform {
  readonly os: Os;
  readonly name: string;
  readonly beta: boolean;
  readonly requires: string;
  readonly files: readonly DownloadFile[];
  readonly note?: string;
}

export const RELEASES_URL = 'https://github.com/pratyush618/carrova/releases/latest';

export const PLATFORMS: readonly Platform[] = [
  {
    os: 'linux',
    name: 'Linux',
    beta: false,
    requires: 'Ubuntu 22.04+, Debian 12+, Fedora, openSUSE · x86_64 and arm64',
    files: [
      { label: 'AppImage', suffix: '_amd64.AppImage' },
      { label: '.deb', suffix: '_amd64.deb' },
      { label: '.rpm', suffix: '.x86_64.rpm' },
      { label: 'AppImage arm64', suffix: '_aarch64.AppImage' },
      { label: '.deb arm64', suffix: '_arm64.deb' },
      { label: '.rpm arm64', suffix: '.aarch64.rpm' },
    ],
  },
  {
    os: 'windows',
    name: 'Windows',
    beta: true,
    requires: 'Windows 10 and 11 · x64',
    files: [
      { label: 'Installer', suffix: '_x64-setup.exe' },
      { label: '.msi', suffix: '_x64_en-US.msi' },
    ],
    note: 'Not signed yet. On the SmartScreen prompt, choose More info, then Run anyway.',
  },
  {
    os: 'mac',
    name: 'macOS',
    beta: true,
    requires: 'macOS 11+ · Intel and Apple silicon',
    files: [{ label: '.dmg', suffix: '_universal.dmg' }],
    note: 'Not notarized yet. Open it once, then choose Open Anyway in System Settings → Privacy & Security.',
  },
  {
    os: 'android',
    name: 'Android',
    beta: true,
    requires: 'Android 13 or newer',
    files: [{ label: '.apk', suffix: '_android-arm64.apk' }],
  },
];

/** Best guess at the visitor's OS. Android before Linux: its user agent says both. */
export function detectOs(userAgent: string, platform = ''): Os | undefined {
  const s = `${platform} ${userAgent}`.toLowerCase();
  if (s.includes('android')) return 'android';
  if (/iphone|ipad|ipod/.test(s)) return undefined;
  if (s.includes('win')) return 'windows';
  if (s.includes('mac')) return 'mac';
  if (/linux|x11|cros/.test(s)) return 'linux';
  return undefined;
}

/**
 * CPU architecture from the user agent, when it says.
 * shortcut: Chrome's reduced UA reports "Linux x86_64" on every Linux; read
 * `userAgentData.getHighEntropyValues(['architecture'])` if ARM Chrome users get the wrong file.
 */
export function detectArch(userAgent: string, platform = ''): Arch | undefined {
  const s = `${platform} ${userAgent}`.toLowerCase();
  if (/aarch64|arm64|armv8/.test(s)) return 'arm64';
  if (/x86_64|amd64|x64|win64|wow64/.test(s)) return 'x64';
  return undefined;
}

/** The one file to offer first. Linux ships per architecture, so unknown arch means let the visitor pick. */
export function primaryFile(platform: Platform, arch: Arch | undefined): DownloadFile | undefined {
  if (platform.os !== 'linux') return platform.files[0];
  if (arch === undefined) return undefined;
  const suffix = arch === 'arm64' ? '_aarch64.AppImage' : '_amd64.AppImage';
  return platform.files.find((f) => f.suffix === suffix);
}

/** Direct download URL for a file, or the releases page when the asset isn't known. */
export function fileUrl(assets: readonly ReleaseAsset[], file: DownloadFile): string {
  return assets.find((a) => a.name.endsWith(file.suffix))?.browser_download_url ?? RELEASES_URL;
}

import { describe, expect, it } from 'vitest';
import { detectOs, fileUrl, PLATFORMS, RELEASES_URL } from '../../src/download/platform';

describe('detectOs', () => {
  it.each([
    ['Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0', 'linux'],
    ['Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/131 Mobile', 'android'],
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131', 'windows'],
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 Safari/605', 'mac'],
    ['Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Safari/604.1', undefined],
  ])('%s → %s', (ua, os) => {
    expect(detectOs(ua)).toBe(os);
  });
});

describe('fileUrl', () => {
  const assets = [
    'Carrova_0.3.1_amd64.deb',
    'Carrova_0.3.1_amd64.deb.sig',
    'Carrova_0.3.1_arm64.deb',
    'Carrova-0.3.1-1.x86_64.rpm',
    'Carrova_0.3.1_x64-setup.exe',
  ].map((name) => ({ name, browser_download_url: `https://dl/${name}` }));

  it('matches the exact package, not its signature or another arch', () => {
    const linux = PLATFORMS.find((p) => p.os === 'linux');
    const deb = linux?.files.find((f) => f.label === '.deb');
    const rpm = linux?.files.find((f) => f.label === '.rpm');
    expect(deb && fileUrl(assets, deb)).toBe('https://dl/Carrova_0.3.1_amd64.deb');
    expect(rpm && fileUrl(assets, rpm)).toBe('https://dl/Carrova-0.3.1-1.x86_64.rpm');
  });

  it('falls back to the releases page for unknown assets', () => {
    expect(fileUrl([], { label: '.dmg', suffix: '_universal.dmg' })).toBe(RELEASES_URL);
  });
});

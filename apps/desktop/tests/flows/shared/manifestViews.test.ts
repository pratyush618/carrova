import { describe, expect, it } from 'vitest';
import { sourceComputer, thisComputer } from '@/flows/shared/manifestViews';
import { plan } from '@/ipc/fixtures/plan';

describe('manifest views', () => {
  it('names the source from its manifest, and nothing before there is a plan', () => {
    expect(sourceComputer({ ...plan, source: { os: 'mac_os', device_name: 'ana-mbp' } })).toEqual({
      name: 'ana-mbp',
      os: 'mac_os',
    });
    expect(sourceComputer({ ...plan, source: { os: 'linux', device_name: null } })).toEqual({
      name: null,
      os: 'linux',
    });
    expect(sourceComputer(null)).toEqual({ name: null, os: null });
  });

  it('describes this computer once the engine answered', () => {
    const about = {
      app_version: '1',
      engine_version: '1',
      device_name: 'ana-laptop',
      os: 'linux',
      status_tray: true,
    };
    expect(thisComputer({ ...about, os: 'linux' })).toEqual({ name: 'ana-laptop', os: 'linux' });
    expect(thisComputer(null)).toEqual({ name: null, os: null });
  });
});

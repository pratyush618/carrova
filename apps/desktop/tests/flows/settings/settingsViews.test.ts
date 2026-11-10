import { describe, expect, it } from 'vitest';
import { idle } from '@/flows/remote';
import { recipeCheck, settingsModel, updateModel } from '@/flows/settings/settingsViews';

const error = { code: 'network', message: 'Offline', hint: null } as const;

describe('settings views', () => {
  it('says "try again later" when the recipe feed is unreachable', () => {
    expect(recipeCheck({ status: 'ready', value: { kind: 'retry_later' } })).toEqual({
      status: 'retry_later',
    });
    expect(recipeCheck({ status: 'ready', value: { kind: 'updated', version: 4 } })).toEqual({
      status: 'updated',
      version: 4,
    });
  });

  it('follows the system theme and motion unless the user chose', () => {
    const model = settingsModel(
      {
        settings: {
          auto_check_recipes: true,
          auto_check_app: false,
          theme: null,
          reduce_motion: true,
          move_defaults: {
            conflict_rule: 'replace',
            retention: 'until_dismissed',
            include_caches: false,
          },
        },
        consent: 'unset',
        reports: [{ id: '1', when: 0, kind: 'error', headline: 'Boom' }],
        recipes: null,
        check: idle,
        update: {
          status: 'ready',
          value: {
            status: 'available',
            version: '0.3.0',
            current_version: '0.2.0',
            notes: null,
            date: null,
          },
        },
      },
      new Date(),
    );
    expect(model).toMatchObject({ appearance: 'system', motion: 'reduce', errorReports: false });
    expect(model.update).toEqual({ version: '0.3.0' });
    expect(model.reports).toHaveLength(1);
  });

  it('walks the update dialog from offer to installed, and shows failures', () => {
    const offer = {
      status: 'ready',
      value: {
        status: 'available',
        version: '0.3.0',
        current_version: '0.2.0',
        notes: 'New',
        date: null,
      },
    } as const;
    expect(updateModel(offer, idle, null)).toEqual({
      status: 'available',
      version: '0.3.0',
      notes: 'New',
    });
    expect(updateModel(offer, { status: 'loading', last: null }, '0.3.0').status).toBe(
      'installing',
    );
    expect(updateModel(offer, { status: 'ready', value: undefined }, '0.3.0').status).toBe(
      'installed',
    );
    expect(updateModel({ status: 'failed', error, last: null }, idle, null)).toEqual({
      status: 'failed',
      error,
    });
  });
});

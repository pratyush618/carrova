import { describe, expect, it } from 'vitest';
import { idle } from '@/flows/remote';
import { recipeCheck, settingsModel, updateModel } from '@/flows/settings/settingsViews';

const error = { code: 'network', message: 'Offline', hint: null } as const;

describe('settings views', () => {
  it('says "try again later" when the recipe feed is unreachable', () => {
    expect(recipeCheck({ status: 'ready', value: { kind: 'retry_later' } }, idle)).toEqual({
      status: 'retry_later',
    });
    expect(
      recipeCheck({ status: 'ready', value: { kind: 'up_to_date', version: 3 } }, idle),
    ).toEqual({ status: 'up_to_date' });
  });

  it('says a rolled-back list is not offered again, not that it is up to date', () => {
    expect(recipeCheck({ status: 'ready', value: { kind: 'refused', version: 5 } }, idle)).toEqual({
      status: 'refused',
      version: 5,
    });
  });

  it('offers a newer list, then installs it only when asked', () => {
    const offered = { status: 'ready', value: { kind: 'available', version: 4 } } as const;
    expect(recipeCheck(offered, idle)).toEqual({ status: 'available', version: 4 });
    expect(recipeCheck(offered, { status: 'loading', last: null })).toEqual({
      status: 'installing',
      version: 4,
    });
    expect(
      recipeCheck(offered, { status: 'ready', value: { kind: 'updated', version: 4 } }),
    ).toEqual({ status: 'updated', version: 4 });
    expect(recipeCheck(offered, { status: 'failed', error, last: null })).toEqual({
      status: 'failed',
      error,
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
        install: idle,
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

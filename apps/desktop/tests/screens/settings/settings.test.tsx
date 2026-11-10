import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { SettingsScreen } from '@/screens/settings/SettingsScreen';
import { settingsModel } from '@/screens/settings/fixtures';
import type { SettingsIntents } from '@/screens/settings/model';

const settingsOn = (): SettingsIntents => ({
  onAppearance: vi.fn(),
  onMotion: vi.fn(),
  onAutoCheckRecipes: vi.fn(),
  onAutoCheckApp: vi.fn(),
  onCheckRecipes: vi.fn(),
  onErrorReports: vi.fn(),
  onOpenReport: vi.fn(),
  onClearReports: vi.fn(),
  onUpdates: vi.fn(),
  onAbout: vi.fn(),
  onMoveDefaults: vi.fn(),
});

describe('Settings', () => {
  it('picks move defaults with the house segmented control, no native select', async () => {
    const user = userEvent.setup();
    const on = settingsOn();
    renderScreen(<SettingsScreen model={settingsModel} on={on} />);
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();

    const rule = screen.getByRole('radiogroup', { name: 'When a file already exists' });
    expect(screen.getByRole('radio', { name: 'Replace' })).toHaveAttribute('aria-checked', 'true');
    await user.click(screen.getByRole('radio', { name: 'Keep both' }));
    expect(on.onMoveDefaults).toHaveBeenCalledWith({ conflict_rule: 'keep-both' });
    expect(rule).toBeInTheDocument();

    screen.getByRole('radiogroup', { name: 'Keep the undo point' });
    await user.click(screen.getByRole('radio', { name: 'Until the move succeeds' }));
    expect(on.onMoveDefaults).toHaveBeenCalledWith({ retention: 'discard_on_success' });
  });
});

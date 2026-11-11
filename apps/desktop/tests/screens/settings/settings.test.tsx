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
  onInstallRecipes: vi.fn(),
  onErrorReports: vi.fn(),
  onOpenReport: vi.fn(),
  onClearReports: vi.fn(),
  onUpdates: vi.fn(),
  onAbout: vi.fn(),
  onMoveDefaults: vi.fn(),
  onStartInManual: vi.fn(),
  onFileManager: vi.fn(),
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
    await user.click(screen.getByRole('radio', { name: 'For 30 days' }));
    expect(on.onMoveDefaults).toHaveBeenCalledWith({ retention: { keep_for: { days: 30 } } });
  });

  it('offers a newer recipe list inline and installs it only on request', async () => {
    const user = userEvent.setup();
    const on = settingsOn();
    const recipes = settingsModel.recipes;
    if (recipes === null) throw new Error('fixture');
    const model = {
      ...settingsModel,
      recipes: { ...recipes, check: { status: 'available', version: 13 } as const },
    };
    renderScreen(<SettingsScreen model={model} on={on} />);
    expect(screen.getByRole('status')).toHaveTextContent('List 13 is ready.');
    expect(screen.queryByRole('button', { name: 'Check now' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Install update' }));
    expect(on.onInstallRecipes).toHaveBeenCalledOnce();
    expect(on.onCheckRecipes).not.toHaveBeenCalled();
  });

  it('explains a rolled-back recipe list and still offers "Check now"', () => {
    const recipes = settingsModel.recipes;
    if (recipes === null) throw new Error('fixture');
    const model = {
      ...settingsModel,
      recipes: { ...recipes, check: { status: 'refused', version: 5 } as const },
    };
    renderScreen(<SettingsScreen model={model} on={settingsOn()} />);
    expect(screen.getByRole('status')).toHaveTextContent(
      "Version 5 was rolled back, so it isn't offered again.",
    );
    expect(screen.queryByRole('button', { name: 'Install update' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check now' })).toBeEnabled();
  });
  it('settings toggle reflects status and calls setFileManagerEntries', async () => {
    const user = userEvent.setup();
    const on = settingsOn();
    const skipped = 'Thunar: uca.xml is not valid; left unchanged';
    renderScreen(
      <SettingsScreen
        model={{ ...settingsModel, fileManager: { on: true, skipped: [skipped], failed: false } }}
        on={on}
      />,
    );
    const entries = screen.getByRole('switch', {
      name: 'Show “Send with Carrova” in the file manager',
    });
    expect(entries).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByText(skipped)).toBeInTheDocument();
    await user.click(entries);
    expect(on.onFileManager).toHaveBeenCalledWith(false);

    const manual = screen.getByRole('switch', { name: 'Start sends in Manual mode' });
    expect(manual).toHaveAttribute('aria-checked', 'false');
    await user.click(manual);
    expect(on.onStartInManual).toHaveBeenCalledWith(true);
  });

  it('hides the file-manager row where the system has none, and says when a change failed', () => {
    const { unmount } = renderScreen(
      <SettingsScreen model={{ ...settingsModel, fileManager: null }} on={settingsOn()} />,
    );
    expect(screen.queryByRole('switch', { name: /file manager/ })).not.toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Start sends in Manual mode' })).toBeInTheDocument();
    unmount();
    renderScreen(
      <SettingsScreen
        model={{ ...settingsModel, fileManager: { on: false, skipped: [], failed: true } }}
        on={settingsOn()}
      />,
    );
    expect(screen.getByRole('status')).toHaveTextContent(
      "Couldn't change the file manager menu. Try again, or restart Carrova.",
    );
  });
});

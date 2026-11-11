import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { SettingsScreen } from '@/screens/settings/SettingsScreen';
import { settingsModel } from '@/screens/settings/fixtures';
import type { SettingsIntents, SettingsModel } from '@/screens/settings/model';
import type { SettingsSection } from '@/screens/settings/sections';

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
  onBackground: vi.fn(),
  onAlerts: vi.fn(),
  onPause: vi.fn(),
  onSection: vi.fn(),
  onBack: vi.fn(),
});

/** The page opened on `section`. */
const inSection = (
  section: SettingsSection,
  on: SettingsIntents,
  model: Partial<SettingsModel> = {},
) => <SettingsScreen section={section} model={{ ...settingsModel, ...model }} on={on} />;

describe('Settings', () => {
  it('swaps the sidebar for the sections, with a back button', async () => {
    const user = userEvent.setup();
    const on = settingsOn();
    renderScreen(<SettingsScreen section="general" model={settingsModel} on={on} />);
    const sections = screen.getByRole('navigation', { name: 'Settings sections' });
    expect(
      within(sections)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['General', 'Sending', 'Notifications', 'Moves', 'Privacy', 'Updates1', 'About']);
    expect(screen.queryByRole('navigation', { name: 'Main' })).not.toBeInTheDocument();
    await user.click(within(sections).getByRole('button', { name: 'Moves' }));
    expect(on.onSection).toHaveBeenCalledWith('moves');
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    expect(on.onBack).toHaveBeenCalledOnce();
  });

  it('shows only the rows of the open section', () => {
    renderScreen(inSection('general', settingsOn()));
    for (const name of ['Reduce motion', 'Keep running when the window closes', 'Start at login'])
      expect(screen.getByText(name)).toBeInTheDocument();
    expect(screen.getByText('Theme')).toBeInTheDocument();
    expect(screen.getByText('Use as little as possible in the background')).toBeInTheDocument();
    expect(screen.queryByText('Start sends in Manual mode')).not.toBeInTheDocument();
    expect(screen.queryByText('Bring caches')).not.toBeInTheDocument();
  });

  it('explains that closing minimises where there is no tray', () => {
    renderScreen(inSection('general', settingsOn(), { minimisesOnClose: true }));
    expect(
      screen.getByText('No tray on this desktop, so closing minimises Carrova instead.'),
    ).toBeInTheDocument();
    expect(screen.queryByText(/moves to the tray/)).not.toBeInTheDocument();
  });

  it('saves background changes as a patch', async () => {
    const user = userEvent.setup();
    const on = settingsOn();
    renderScreen(inSection('general', on));
    await user.click(screen.getByRole('switch', { name: 'Start at login' }));
    expect(on.onBackground).toHaveBeenCalledWith({ start_at_login: true });
  });

  it('dims a sub-setting while the row above it is off', () => {
    const alerts = { ...settingsModel.alerts, system: false };
    renderScreen(inSection('notifications', settingsOn(), { alerts }));
    expect(
      screen.getByRole('switch', { name: 'Wake the screen for send requests' }),
    ).toBeDisabled();
    expect(screen.getByRole('switch', { name: 'Show names on the lock screen' })).toBeDisabled();
    expect(screen.getByRole('switch', { name: 'System notifications' })).toBeEnabled();
  });

  it('shows when a running pause ends', () => {
    const pausedUntil = new Date(settingsModel.now.getTime() + 3_600_000);
    renderScreen(inSection('notifications', settingsOn(), { pausedUntil }));
    expect(screen.getByRole('radio', { name: 'Off' })).toHaveAttribute('aria-checked', 'false');
    expect(screen.getByText(/Paused until/)).toBeInTheDocument();
  });

  it('picks how each kind of alert reaches you', async () => {
    const user = userEvent.setup();
    const on = settingsOn();
    renderScreen(inSection('notifications', on));
    const nearby = screen.getByRole('radiogroup', { name: 'A known device comes nearby' });
    expect(within(nearby).getByRole('radio', { name: 'Bell only' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await user.click(within(nearby).getByRole('radio', { name: 'Pop-up and bell' }));
    expect(on.onAlerts).toHaveBeenCalledWith({ nearby: 'popup_and_bell' });
    await user.click(screen.getByRole('radio', { name: '1 hour' }));
    expect(on.onPause).toHaveBeenCalledWith('hour');
  });

  it('searches every section and opens the one a result belongs to', async () => {
    const user = userEvent.setup();
    const on = settingsOn();
    renderScreen(<SettingsScreen section="general" model={settingsModel} on={on} />);
    await user.type(screen.getByRole('searchbox', { name: 'Search settings' }), 'wake');
    expect(screen.getByText('1 result for “wake”')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /Wake the screen/ }).querySelector('mark'),
    ).toHaveTextContent('Wake');
    expect(screen.queryByText('Reduce motion')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Wake the screen/ }));
    expect(on.onSection).toHaveBeenCalledWith('notifications');
    expect(screen.queryByText('1 result for “wake”')).not.toBeInTheDocument();
  });

  it('picks move defaults with the house segmented control, no native select', async () => {
    const user = userEvent.setup();
    const on = settingsOn();
    renderScreen(inSection('moves', on));
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
    const model = { recipes: { ...recipes, check: { status: 'available', version: 13 } as const } };
    renderScreen(inSection('updates', on, model));
    expect(screen.getByRole('status')).toHaveTextContent('List 13 is ready.');
    expect(screen.queryByRole('button', { name: 'Check now' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Install update' }));
    expect(on.onInstallRecipes).toHaveBeenCalledOnce();
    expect(on.onCheckRecipes).not.toHaveBeenCalled();
  });

  it('explains a rolled-back recipe list and still offers "Check now"', () => {
    const recipes = settingsModel.recipes;
    if (recipes === null) throw new Error('fixture');
    const model = { recipes: { ...recipes, check: { status: 'refused', version: 5 } as const } };
    renderScreen(inSection('updates', settingsOn(), model));
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
      inSection('sending', on, { fileManager: { on: true, skipped: [skipped], failed: false } }),
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
    const { unmount } = renderScreen(inSection('sending', settingsOn(), { fileManager: null }));
    expect(screen.queryByRole('switch', { name: /file manager/ })).not.toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Start sends in Manual mode' })).toBeInTheDocument();
    unmount();
    renderScreen(
      inSection('sending', settingsOn(), { fileManager: { on: false, skipped: [], failed: true } }),
    );
    expect(screen.getByRole('status')).toHaveTextContent(
      "Couldn't change the file manager menu. Try again, or restart Carrova.",
    );
  });
});

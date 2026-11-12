import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { buildChrome } from '@/app/shell/chrome';
import { i18n } from '@/i18n/i18n';
import { nearbyEmpty, nearbyList, nearbyMap, nearbySelected } from '@/screens/nearby/fixtures';
import type { NearbyIntents, NearbyModel, NearbyView } from '@/screens/nearby/model';
import { NearbyScreen } from '@/screens/nearby/NearbyScreen';
import { ChromeContext, Window } from '@/ui';

const nearbyOn = (): NearbyIntents => ({
  onView: vi.fn(),
  onSelect: vi.fn(),
  onVisibility: vi.fn(),
  onSettings: vi.fn(),
  onSendWithCode: vi.fn(),
  onWake: vi.fn(),
  onForget: vi.fn(),
  onDismissWake: vi.fn(),
});

const page = (model: NearbyModel, on = nearbyOn()) => <NearbyScreen model={model} on={on} />;

describe('Nearby', () => {
  it('visibility shows once on the page', () => {
    renderScreen(page(nearbyMap));
    expect(document.body.textContent.match(/Visible to/g)).toHaveLength(1);
    expect(
      screen.getByRole('button', { name: /Visible to known devices as ezio-thinkpad/ }),
    ).toBeInTheDocument();
  });

  it('sidebar shows a dot, not text', () => {
    const chrome = buildChrome(
      { kind: 'hub', page: 'nearby' },
      i18n.t,
      () => undefined,
      null,
      undefined,
      true,
    );
    render(
      <ChromeContext value={chrome}>
        <Window overlays={[]}>{page(nearbyMap)}</Window>
      </ChromeContext>,
    );
    const link = within(screen.getByRole('navigation', { name: 'Main' })).getByRole('button', {
      name: 'Nearby',
    });
    expect(link).toHaveTextContent(/^Nearby$/);
    expect(link.querySelector('.p-side__live')).not.toBeNull();
  });

  it('chooses who can see this computer from the pill', async () => {
    const user = userEvent.setup();
    const on = nearbyOn();
    renderScreen(page(nearbyMap, on));
    const pill = screen.getByRole('button', { name: /Visible to/ });
    await user.click(pill);
    expect(screen.getByRole('button', { name: /^Known devices/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await user.click(screen.getByRole('button', { name: /^Everyone for 10 minutes/ }));
    expect(on.onVisibility).toHaveBeenCalledWith('everyone');
    expect(pill).toHaveAttribute('aria-expanded', 'false');
    await user.click(pill);
    await user.click(screen.getByRole('button', { name: /^Nearby settings/ }));
    expect(on.onSettings).toHaveBeenCalledOnce();
  });

  it('away devices are faded and offer Forget', async () => {
    const user = userEvent.setup();
    const on = nearbyOn();
    renderScreen(page(nearbyList, on));
    const away = screen.getByRole('list', { name: 'Known, not here' });
    for (const row of within(away).getAllByRole('listitem')) expect(row).toHaveClass('p-row--off');
    await user.click(screen.getByRole('button', { name: 'Forget: Work laptop' }));
    expect(on.onForget).toHaveBeenCalledWith(expect.objectContaining({ id: 'w4' }));
    const here = screen.getByRole('list', { name: 'Here now' });
    expect(within(here).queryByRole('button', { name: /^Forget/ })).not.toBeInTheDocument();
    expect(within(here).getAllByRole('button', { name: /^Send with code/ })).toHaveLength(3);
  });

  it('wake shown only with a hint', async () => {
    const user = userEvent.setup();
    const on = nearbyOn();
    renderScreen(page(nearbyList, on));
    expect(screen.queryByRole('button', { name: 'Wake: Work laptop' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Wake: Old ThinkPad' }));
    expect(on.onWake).toHaveBeenCalledWith(expect.objectContaining({ id: 'o3' }));
  });

  it('opens a device card on the map with what you have done together', () => {
    renderScreen(page(nearbySelected));
    const card = screen.getByRole('region', { name: "Ana's laptop" });
    expect(within(card).getByText('Moves together')).toBeInTheDocument();
    expect(within(card).getByText('7F3A·19C2')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Ana's laptop/, pressed: true })).toBeInTheDocument();
  });

  it('moves focus to the card when a device is chosen on the map', async () => {
    const user = userEvent.setup();
    function Choosing() {
      const [selected, setSelected] = useState<string | null>(null);
      return page({ ...nearbyMap, selected }, { ...nearbyOn(), onSelect: setSelected });
    }
    renderScreen(<Choosing />);
    await user.click(screen.getByRole('button', { name: /Desk PC/ }));
    expect(screen.getByRole('heading', { level: 2, name: 'Desk PC' })).toHaveFocus();
  });

  it('says it is still looking, and offers a code, while nobody answers', async () => {
    const user = userEvent.setup();
    const on = nearbyOn();
    renderScreen(page(nearbyEmpty, on));
    expect(
      screen.getByRole('heading', { name: 'Looking for Carrova devices' }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Send with a code' }));
    expect(on.onSendWithCode).toHaveBeenCalledOnce();
  });

  it('no pulse under reduced motion', () => {
    const { container, unmount } = renderScreen(page(nearbyMap));
    expect(container.querySelector('.s-near__pulse')).not.toBeNull();
    unmount();
    const still = renderScreen(page(nearbyMap), { reduceMotion: true });
    expect(still.container.querySelector('.s-near__pulse')).toBeNull();
  });

  it('does not replay the pulse after leaving the map before it ended', async () => {
    const user = userEvent.setup();
    function Switching() {
      const [view, setView] = useState<NearbyView>('map');
      return page({ ...nearbyMap, view }, { ...nearbyOn(), onView: setView });
    }
    const { container } = renderScreen(<Switching />);
    expect(container.querySelector('.s-near__pulse')).not.toBeNull();
    await user.click(screen.getByRole('radio', { name: 'List' }));
    await user.click(screen.getByRole('radio', { name: 'Map' }));
    expect(container.querySelector('.s-near__pulse')).toBeNull();
  });

  it('says when a change to the devices was refused', () => {
    renderScreen(page({ ...nearbyList, failed: 'forget' }));
    expect(screen.getByRole('status')).toHaveTextContent("Couldn't forget the device. Try again.");
  });

  it('keeps the keyboard in the pill: Tab reaches the choices, closing returns to the pill', async () => {
    const user = userEvent.setup();
    renderScreen(page(nearbyMap));
    const pill = screen.getByRole('button', { name: /Visible to/ });
    await user.click(pill);
    await user.tab();
    expect(screen.getByRole('button', { name: /^No one/ })).toHaveFocus();
    await user.keyboard('{Escape}');
    expect(pill).toHaveFocus();
    await user.click(pill);
    await user.click(screen.getByRole('button', { name: /^Everyone for 10 minutes/ }));
    expect(pill).toHaveFocus();
  });

  it('says no one can see this computer without naming it', () => {
    renderScreen(page({ ...nearbyMap, visibility: { choice: 'no_one', until: null } }));
    expect(screen.getByRole('button', { name: 'Visible to no one' })).toBeInTheDocument();
  });
});

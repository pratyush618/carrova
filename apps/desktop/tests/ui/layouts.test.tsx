import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ActionBar,
  ChromeContext,
  FlowLayout,
  HeaderBar,
  HubLayout,
  Manifest,
  Window,
  type Chrome,
} from '@/ui';
import { renderScreen, testChrome } from '../support/render';

afterEach(() => {
  vi.unstubAllGlobals();
});

function stubWideWindow() {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: query.includes('min-width: 960px'),
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }));
}

const rail = (
  <Manifest
    from={{ name: 'ezio-thinkpad', note: 'This computer', os: { kind: 'linux', label: 'Ubuntu' } }}
    to={{ name: 'The other computer', note: "You'll pair in the next step" }}
    facts={[{ label: 'Apps', value: '11 of 15' }]}
    promise="Nothing is written until someone approves."
  />
);

function Flow() {
  return (
    <FlowLayout rail={rail} actions={<ActionBar end={<button type="button">Next</button>} />}>
      <h1>Pick what moves</h1>
    </FlowLayout>
  );
}

describe('FlowLayout', () => {
  it('starts with a Back arrow that leaves the flow', async () => {
    const onClose = vi.fn<() => void>();
    render(
      <ChromeContext
        value={{ ...testChrome, flow: { name: 'Send', steps: ['Pick'], step: 1, onClose } }}
      >
        <Window>
          <Flow />
        </Window>
      </ChromeContext>,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(onClose).toHaveBeenCalledOnce();
    expect(screen.queryByRole('button', { name: 'Close and go home' })).not.toBeInTheDocument();
  });

  it('names every step and marks the current one', () => {
    renderScreen(<Flow />);
    const steps = screen.getByRole('list', { name: 'Step 1 of 4' });
    expect(
      within(steps)
        .getAllByRole('listitem')
        .map((li) => li.textContent),
    ).toEqual(['1Pick', '2Pair', '3Their review', '4Move']);
    expect(within(steps).getByText('Pick').closest('li')).toHaveAttribute('aria-current', 'step');
  });

  it('lets a screen move the current step on (stepNow)', () => {
    renderScreen(
      <FlowLayout stepNow={2} actions={null}>
        <h1>Waiting for their review</h1>
      </FlowLayout>,
    );
    expect(screen.getByRole('list', { name: 'Step 3 of 4' })).toBeInTheDocument();
  });

  it('shows the manifest beside the main column on wide windows', () => {
    stubWideWindow();
    renderScreen(<Flow />);
    expect(screen.getByRole('complementary', { name: 'This move' })).toHaveTextContent(
      'ezio-thinkpad',
    );
    expect(screen.queryByRole('button', { name: 'This move' })).not.toBeInTheDocument();
  });

  it('folds the manifest into a header button and side sheet on narrow windows', async () => {
    const user = userEvent.setup();
    renderScreen(<Flow />);
    expect(screen.queryByRole('complementary')).not.toBeInTheDocument();

    const open = screen.getByRole('button', { name: 'This move' });
    await user.click(open);
    const sheet = screen.getByRole('dialog', { name: 'This move' });
    expect(sheet).toHaveTextContent('11 of 15');
    expect(screen.getByRole('button', { name: 'Next' }).closest('[inert]')).not.toBeNull();

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(open).toHaveFocus();
  });
});

describe('Manifest', () => {
  it('offers a named Change link only for facts that can change', async () => {
    const onChange = vi.fn();
    render(
      <Manifest
        from={{
          name: 'ezio-thinkpad',
          note: 'This computer',
          os: { kind: 'linux', label: 'Ubuntu' },
        }}
        to={{ name: 'ezio-mbp', note: 'Paired', os: { kind: 'mac_os', label: 'macOS 15' } }}
        facts={[
          { label: 'Apps', value: '11 of 15' },
          { label: 'Keys', value: 'Left behind', onChange },
        ]}
        promise="Every move can be undone."
      />,
    );
    expect(screen.getAllByRole('button')).toHaveLength(1);
    await userEvent.click(screen.getByRole('button', { name: 'Change: Keys' }));
    expect(onChange).toHaveBeenCalledOnce();
  });
});

describe('HubLayout', () => {
  it('marks the current place in the sidebar and shows the bundle state at its foot', () => {
    const chrome: Chrome = {
      hubLinks: [
        { id: 'home', label: 'Home', icon: 'home', current: true, onSelect: () => undefined },
        {
          id: 'history',
          label: 'History',
          icon: 'history',
          current: false,
          onSelect: () => undefined,
        },
      ],
      hubStatus: {
        title: 'Recipes up to date',
        tone: 'success',
        detail: 'Bundle 2026.11.3 · signed',
      },
      flow: null,
    };
    render(
      <ChromeContext value={chrome}>
        <Window>
          <HubLayout>
            <h1>Home</h1>
          </HubLayout>
        </Window>
      </ChromeContext>,
    );
    const nav = screen.getByRole('navigation', { name: 'Main' });
    expect(within(nav).getByRole('button', { name: 'Home' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(within(nav).getByRole('button', { name: 'History' })).not.toHaveAttribute(
      'aria-current',
    );
    expect(nav).toHaveTextContent('Recipes up to date');
  });
});

describe('HeaderBar', () => {
  it('renders the tools, then the bell', () => {
    render(
      <HeaderBar
        tools={<button type="button">Filter</button>}
        bell={<button type="button">Bell</button>}
      />,
    );
    const names = screen.getAllByRole('button').map((b) => b.textContent);
    expect(names).toEqual(['Filter', 'Bell']);
  });
});

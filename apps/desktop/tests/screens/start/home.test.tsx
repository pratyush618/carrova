import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { HomeScreen } from '@/screens/start/HomeScreen';
import { homeFirst, homeReturning } from '@/screens/start/fixtures';

const homeOn = () => ({
  onSend: vi.fn(),
  onReceive: vi.fn(),
  onUseFile: vi.fn(),
  onUndo: vi.fn(),
  onHistory: vi.fn(),
  onErrorReports: vi.fn(),
  onFileManager: vi.fn(),
});

describe('Home', () => {
  it('teaches a first run: hint, three steps, the promise and the report question', async () => {
    const user = userEvent.setup();
    const on = homeOn();
    renderScreen(<HomeScreen model={homeFirst} on={on} />);
    expect(
      screen.getByRole('heading', { level: 1, name: 'Bring your setup with you.' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('list', { name: 'How a move works' }).children).toHaveLength(3);

    await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(screen.queryByText('Start here on your old computer')).not.toBeInTheDocument();

    expect(
      screen.getByRole('heading', { name: 'Send error reports if Carrova crashes?' }),
    ).toBeInTheDocument();
    const whatsSent = screen.getByRole('button', { name: "What's sent" });
    const body = screen.getByText(/You see each report before it goes/);
    expect(body).not.toBeVisible();
    await user.click(whatsSent);
    expect(whatsSent).toHaveAttribute('aria-expanded', 'true');
    expect(body).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'No thanks' }));
    expect(on.onErrorReports).toHaveBeenCalledWith(false);
  });

  it('lists recent moves with Undo for returning users, and no report question', async () => {
    const user = userEvent.setup();
    const on = homeOn();
    renderScreen(<HomeScreen model={homeReturning} on={on} />);
    expect(
      screen.getByRole('heading', { level: 1, name: 'Good to see you again.' }),
    ).toBeInTheDocument();
    expect(screen.queryByRole('list', { name: 'How a move works' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: /error reports/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Undo: Received from ezio-desktop' }));
    expect(on.onUndo).toHaveBeenCalledWith('s-1108');
    await user.click(screen.getByRole('button', { name: 'See all' }));
    expect(on.onHistory).toHaveBeenCalledOnce();
  });

  it('starts each verb from its key, but not while typing or under a dialog', async () => {
    const user = userEvent.setup();
    const on = homeOn();
    renderScreen(
      <>
        <HomeScreen model={homeReturning} on={on} />
        <input aria-label="Typing" />
      </>,
    );
    await user.keyboard('s');
    await user.keyboard('R');
    await user.keyboard('o');
    expect(on.onSend).toHaveBeenCalledOnce();
    expect(on.onReceive).toHaveBeenCalledOnce();
    expect(on.onUseFile).toHaveBeenCalledOnce();

    await user.click(screen.getByRole('textbox', { name: 'Typing' }));
    await user.keyboard('s');
    await user.keyboard('{Control>}r{/Control}');
    expect(on.onSend).toHaveBeenCalledOnce();
    expect(on.onReceive).toHaveBeenCalledOnce();
  });
  it('home asks once about the file manager, in one line under the moves', async () => {
    const user = userEvent.setup();
    const on = homeOn();
    renderScreen(<HomeScreen model={{ ...homeReturning, askFileManager: true }} on={on} />);
    const card = screen.getByRole('region', {
      name: 'Add “Send with Carrova” to your file manager?',
    });
    expect(card).toHaveTextContent(/Right-click files or folders/);
    await user.click(screen.getByRole('button', { name: 'Add it' }));
    expect(on.onFileManager).toHaveBeenCalledWith(true);
    await user.click(screen.getByRole('button', { name: 'Not now' }));
    expect(on.onFileManager).toHaveBeenCalledWith(false);
  });

  it('home_file_manager_card_stays_with_an_error_when_adding_failed', async () => {
    const user = userEvent.setup();
    const on = homeOn();
    renderScreen(
      <HomeScreen
        model={{ ...homeReturning, askFileManager: true, fileManagerFailed: true }}
        on={on}
      />,
    );
    expect(screen.getByRole('alert')).toHaveTextContent(
      "Couldn't add it to the file manager. Try again, or restart Carrova.",
    );
    await user.click(screen.getByRole('button', { name: 'Add it' }));
    expect(on.onFileManager).toHaveBeenCalledWith(true);
  });
});

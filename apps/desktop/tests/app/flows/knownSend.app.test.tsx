import { screen, waitFor, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderApp } from '../../support/app';

const home = () => screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });

/** From Home to "Send your setup to…" with the suggested selection. */
async function chooseComputer(user: UserEvent): Promise<HTMLElement> {
  await home();
  await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
  await user.click(await screen.findByRole('button', { name: 'Choose a computer' }));
  return screen.findByRole('dialog', { name: 'Send your setup to…' });
}

describe('sending to a trusted computer', () => {
  it('asks it without a code, and the move runs once it reviews', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ scenario: 'known-send', returning: true });
    const requestSend = vi.spyOn(port, 'requestSend');
    const runSend = vi.spyOn(port, 'runSend');
    const sheet = await chooseComputer(user);
    expect(within(sheet).queryByText('Pixel 8')).not.toBeInTheDocument();
    await user.click(await within(sheet).findByRole('button', { name: 'Send to Desk PC' }));

    expect(requestSend).toHaveBeenCalledWith(expect.any(String), 'd2');
    expect(await screen.findByText('Your setup is on the new computer.')).toBeInTheDocument();
    expect(runSend).not.toHaveBeenCalled();
  });

  it('offers only "Change what moves" when it declines', async () => {
    const user = userEvent.setup();
    await renderApp({ scenario: 'known-send', returning: true });
    const sheet = await chooseComputer(user);
    await user.click(await within(sheet).findByRole('button', { name: "Send to Ana's laptop" }));

    const declined = await screen.findByRole('alertdialog', { name: "Ana's laptop declined" });
    expect(
      within(declined)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['Close', 'Change what moves']);
    await user.click(within(declined).getByRole('button', { name: 'Change what moves' }));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Choose a computer' })).toBeEnabled();
  });

  it('pairs with a code for a new computer', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ scenario: 'known-send', returning: true });
    const runSend = vi.spyOn(port, 'runSend');
    const sheet = await chooseComputer(user);
    const fresh = within(sheet).getByRole('list', { name: 'Needs a code once' });
    await user.click(within(fresh).getByRole('button', { name: 'Use a code' }));

    await waitFor(() => {
      expect(runSend).toHaveBeenCalledOnce();
    });
  });

  it('shows no code-less entry points when the build has none', async () => {
    const user = userEvent.setup();
    await renderApp({ scenario: 'nearby', returning: true });
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    expect(await screen.findByRole('button', { name: 'Get pairing code' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Choose a computer' })).not.toBeInTheDocument();
  });
});

describe('a move request arriving', () => {
  it('shows on Home with its counts, and Review opens the receive review', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ scenario: 'move-request', returning: true });
    const answer = vi.spyOn(port, 'answerRequest');
    await home();
    await screen.findByText('Desk PC wants to send you its setup');
    expect(screen.getByText(/^12 apps · 340 MB · includes keys · \d:\d\d left$/)).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Review' }));

    expect(answer).toHaveBeenCalledWith(1, 'review');
    expect(await screen.findByRole('button', { name: 'Approve and move' })).toBeInTheDocument();
  });

  it('says so when the request ended before Review could open', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ scenario: 'move-request', returning: true });
    vi.spyOn(port, 'answerRequest').mockResolvedValue(null);
    await home();
    await user.click(await screen.findByRole('button', { name: 'Review' }));

    expect(
      await screen.findByText('The move request from Desk PC ended before Review could open'),
    ).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1, name: 'Good to see you again.' })).toBeVisible();
  });

  it('leaves at once when declined', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ scenario: 'move-request', returning: true });
    const answer = vi.spyOn(port, 'answerRequest');
    await home();
    await screen.findByText('Desk PC wants to send you its setup');
    await user.click(screen.getByRole('button', { name: 'Decline' }));

    expect(answer).toHaveBeenCalledWith(1, 'decline');
    expect(screen.queryByText('Desk PC wants to send you its setup')).not.toBeInTheDocument();
  });

  it('turns Send requests off on the computer’s page', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ scenario: 'known-send', returning: true });
    const setSendRequests = vi.spyOn(port, 'setSendRequests');
    await home();
    await user.click(await screen.findByRole('button', { name: /^Desk PC/ }));
    const toggle = await screen.findByRole('switch', { name: 'Send requests' });
    await user.click(toggle);

    expect(setSendRequests).toHaveBeenCalledWith('d2', false);
    expect(
      await screen.findByText('Off: Desk PC needs a code to send you a move.'),
    ).toBeInTheDocument();
  });

  it('says so when the Send requests change is refused', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ scenario: 'known-send', returning: true });
    vi.spyOn(port, 'setSendRequests').mockRejectedValue({ code: 'io', message: 'no' });
    await home();
    await user.click(await screen.findByRole('button', { name: /^Desk PC/ }));
    await user.click(await screen.findByRole('switch', { name: 'Send requests' }));

    expect(await screen.findByText("Couldn't change that. Try again.")).toBeInTheDocument();
  });
});

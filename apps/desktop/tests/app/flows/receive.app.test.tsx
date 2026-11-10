import { screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderApp } from '../../support/app';

async function toReview(user: UserEvent, code = '7-acid-lemon'): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'What are we doing on this computer?' });
  await user.click(screen.getByRole('button', { name: /Receive/ }));
  await user.type(await screen.findByRole('textbox', { name: 'Pairing code' }), code);
  await user.click(screen.getByRole('button', { name: 'Connect' }));
  // The review is ready once Approve shows (the planning screen has the same title).
  await screen.findByRole('button', { name: 'Approve and move' });
}

/** Sensitive items start unconfirmed: bring every one (each answer re-plans). */
async function bringAll(user: UserEvent): Promise<void> {
  for (;;) {
    const [open] = screen.queryAllByRole('radio', { name: 'Bring them', checked: false });
    if (open === undefined) break;
    await user.click(open);
    await screen.findByRole('button', { name: 'Approve and move' });
  }
  await vi.waitFor(() => {
    expect(screen.getByRole('button', { name: 'Approve and move' })).toBeEnabled();
  });
}

describe('receiving over the LAN', () => {
  it('pairs, plans with the suggested options, applies and lists what is left', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const plan = vi.spyOn(port, 'plan');
    await toReview(user);
    expect(plan.mock.calls[0]?.[1]).toMatchObject({ items: null, default_rule: 'replace' });
    expect(screen.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
    await bringAll(user);

    await user.click(screen.getByRole('button', { name: 'Approve and move' }));
    expect(await screen.findByText('Everything arrived.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo this move' })).toBeInTheDocument();
  });

  it('re-plans with a per-file rule chosen in Every change', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const plan = vi.spyOn(port, 'plan');
    await toReview(user);
    await bringAll(user);
    await user.click(screen.getByRole('button', { name: /See every change/ }));
    const sheet = screen.getByRole('dialog', { name: 'Every change' });
    const [first] = within(sheet).getAllByRole('combobox', { name: /What happens to/ });
    if (first === undefined) throw new Error('no clashes in the fixture');
    await user.selectOptions(first, 'keep-both');

    const last = plan.mock.calls.at(-1)?.[1];
    expect(Object.values(last?.path_rules ?? {})).toEqual(['keep-both']);
  });

  it('leaves a sensitive item behind when the user says so', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const plan = vi.spyOn(port, 'plan');
    await toReview(user);
    const [leave] = screen.getAllByRole('radio', { name: 'Leave behind' });
    if (leave === undefined) throw new Error('no decisions in the fixture');
    await user.click(leave);

    const last = plan.mock.calls.at(-1)?.[1];
    expect(last?.items).not.toBeNull();
    expect(last?.confirmed_sensitive).toEqual([]);
  });

  it('keeps Approve disabled while the plan has a blocker and says why', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true, scenario: 'blocked' });
    await toReview(user);
    expect(screen.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
    expect(screen.getByText(/Neovim is open. Close it, skip it/)).toBeInTheDocument();
  });

  it('shows a failed apply that was rolled back, with the engine reason', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true, scenario: 'apply-fails' });
    await toReview(user);
    await bringAll(user);
    await user.click(screen.getByRole('button', { name: 'Approve and move' }));
    expect(
      await screen.findByRole('heading', { name: 'Something went wrong, so nothing changed' }),
    ).toBeInTheDocument();
    expect(screen.getByText(/keyring locked/)).toBeInTheDocument();
  });

  it('tells a wrong code apart and asks for the new one', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true, scenario: 'wrong-code' });
    await screen.findByRole('heading', { level: 1, name: 'What are we doing on this computer?' });
    await user.click(screen.getByRole('button', { name: /Receive/ }));
    await user.type(await screen.findByRole('textbox', { name: 'Pairing code' }), '7-acid-lemon');
    await user.click(screen.getByRole('button', { name: 'Connect' }));
    expect(await screen.findByText("That code didn't match")).toBeInTheDocument();
  });

  it('says when something typed is not a code at all', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await screen.findByRole('heading', { level: 1, name: 'What are we doing on this computer?' });
    await user.click(screen.getByRole('button', { name: /Receive/ }));
    await user.type(await screen.findByRole('textbox', { name: 'Pairing code' }), 'nonsense');
    await user.click(screen.getByRole('button', { name: 'Connect' }));
    expect(await screen.findByText("That isn't a Carrova code")).toBeInTheDocument();
  });

  it('undoes the move from the arrived screen', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const rollback = vi.spyOn(port, 'rollback');
    await toReview(user);
    await bringAll(user);
    await user.click(screen.getByRole('button', { name: 'Approve and move' }));
    await user.click(await screen.findByRole('button', { name: 'Undo this move' }));
    await user.click(screen.getByRole('button', { name: 'Undo the move' }));

    expect(rollback).toHaveBeenCalledOnce();
    expect(
      await screen.findByText('Undone. This computer is back to how it was.'),
    ).toBeInTheDocument();
  });
});

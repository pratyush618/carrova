import { screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { PlanReview } from '@/bindings/PlanReview';
import { renderApp } from '../../support/app';

async function toReview(user: UserEvent, code = '7-acid-lemon'): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
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
    const edit = vi.spyOn(port, 'editPlan');
    await toReview(user);
    await bringAll(user);
    await user.click(screen.getByRole('button', { name: /See every change/ }));
    const sheet = screen.getByRole('dialog', { name: 'Every change' });
    const [first] = within(sheet).getAllByRole('combobox', { name: /What happens to/ });
    if (first === undefined) throw new Error('no clashes in the fixture');
    await user.selectOptions(first, 'keep-both');

    expect(edit.mock.calls.at(-1)?.[1]).toEqual([
      { kind: 'path_rule', path: expect.any(String) as string, rule: 'keep-both' },
    ]);
    const replanned = (await edit.mock.results.at(-1)?.value) as PlanReview | undefined;
    expect(Object.values(replanned?.plan.options.path_rules ?? {})).toEqual(['keep-both']);
  });

  it('shows the place just chosen when Put elsewhere is reopened before the plan catches up', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    await toReview(user);
    await bringAll(user);
    const editPlan = port.editPlan.bind(port);
    let release = (): void => undefined;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    vi.spyOn(port, 'editPlan').mockImplementationOnce(async (...args) => {
      await held;
      return editPlan(...args);
    });
    await user.click(screen.getByRole('button', { name: /See every change/ }));
    const [remap] = screen.getAllByRole('button', { name: 'Put elsewhere…' });
    if (remap === undefined) throw new Error('nothing to put elsewhere in the fixture');
    await user.click(remap);
    await user.click(screen.getByRole('button', { name: 'Choose a folder' }));
    await user.type(screen.getByRole('textbox', { name: 'Inside it (optional)' }), 'old-laptop');
    await user.click(screen.getByRole('button', { name: 'Use this place' }));
    await user.click(await screen.findByRole('button', { name: /See every change/ }));
    const [again] = screen.getAllByRole('button', { name: 'Put elsewhere…' });
    if (again === undefined) throw new Error('remap row gone');
    await user.click(again);
    await user.click(screen.getByRole('button', { name: 'Choose a folder' }));
    release();

    await vi.waitFor(() => {
      expect(screen.getByRole('textbox', { name: 'Inside it (optional)' })).toHaveValue(
        'old-laptop',
      );
    });
  });

  it('leaves a sensitive item behind when the user says so', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const edit = vi.spyOn(port, 'editPlan');
    await toReview(user);
    const [leave] = screen.getAllByRole('radio', { name: 'Leave behind' });
    if (leave === undefined) throw new Error('no decisions in the fixture');
    await user.click(leave);

    expect(edit.mock.calls.at(-1)?.[1]).toMatchObject([{ kind: 'answer_sensitive', bring: false }]);
    const replanned = (await edit.mock.results.at(-1)?.value) as PlanReview | undefined;
    expect(replanned?.plan.options.items).not.toBeNull();
    expect(replanned?.plan.options.confirmed_sensitive).toEqual([]);
  });

  it('takes back an answer the engine could not plan, says why and waits to approve', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    await toReview(user);
    await bringAll(user);
    vi.spyOn(port, 'editPlan').mockRejectedValueOnce({
      code: 'invalid_selection',
      message: 'the plan changed underneath',
      hint: null,
    });
    const [leave] = screen.getAllByRole('radio', { name: 'Leave behind' });
    if (leave === undefined) throw new Error('no decisions in the fixture');
    await user.click(leave);

    expect(await screen.findByRole('alert')).toHaveTextContent('the plan changed underneath');
    expect(leave).not.toBeChecked();
    expect(screen.getByRole('button', { name: 'Approve and move' })).toBeDisabled();

    await user.click(leave);
    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: 'Approve and move' })).toBeEnabled();
    });
    expect(leave).toBeChecked();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('keeps Approve disabled while the plan has a blocker and asks about it', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true, scenario: 'blocked' });
    await toReview(user);
    expect(screen.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
    expect(screen.getByText('Neovim is open')).toBeInTheDocument();
  });

  it('brings an item from another system only after the "I understand" override', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, scenario: 'cross-os' });
    const edit = vi.spyOn(port, 'editPlan');
    await toReview(user);
    await user.click(screen.getByRole('button', { name: /^Override… / }));
    const dialog = screen.getByRole('alertdialog', { name: /anyway\?$/ });
    const go = within(dialog).getByRole('button', { name: 'Bring anyway' });
    expect(go).toBeDisabled();
    await user.click(within(dialog).getByRole('checkbox', { name: /I understand/ }));
    await user.click(go);

    expect(edit.mock.calls.at(-1)?.[1]).toEqual([
      { kind: 'allow_cross_os', item: 'tmux/config', allow: true },
    ]);
    expect(await screen.findByText('Bringing it anyway')).toBeInTheDocument();
  });

  it("asks this computer to allow a plugin and writes the user's consent", async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, scenario: 'plugin' });
    const edit = vi.spyOn(port, 'editPlan');
    await toReview(user);
    await user.click(screen.getByRole('button', { name: 'Review… (path-rewrite plugin)' }));
    const dialog = screen.getByRole('dialog', { name: 'Allow “path-rewrite”?' });
    await user.click(within(dialog).getByRole('button', { name: 'Allow for this move' }));

    expect(edit.mock.calls.at(-1)?.[1]).toEqual([
      { kind: 'allow_plugin', plugin: 'path-rewrite', allow: true },
    ]);
    expect(await screen.findByText('Allowed for this move')).toBeInTheDocument();
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
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    await user.click(screen.getByRole('button', { name: /Receive/ }));
    await user.type(await screen.findByRole('textbox', { name: 'Pairing code' }), '7-acid-lemon');
    await user.click(screen.getByRole('button', { name: 'Connect' }));
    expect(await screen.findByText("That code didn't match")).toBeInTheDocument();
  });

  it('says when something typed is not a code at all', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
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
    await user.click(screen.getByRole('button', { name: 'Undo move' }));

    expect(rollback).toHaveBeenCalledOnce();
    expect(
      await screen.findByText('Undone. This computer is back to how it was.'),
    ).toBeInTheDocument();
  });
});

describe('receiving Manual picks', () => {
  it('accept all safe picks then approve', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, scenario: 'picks' });
    const edit = vi.spyOn(port, 'editPlan');
    await toReview(user);
    await user.click(screen.getByRole('button', { name: 'Accept all 2 files' }));

    expect(edit.mock.calls.at(-1)?.[1]).toEqual([{ kind: 'accept_safe_picks' }]);
    expect(await screen.findByText('Accepted')).toBeInTheDocument();
    // The SSH key pick still asks on its own.
    expect(screen.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
    await bringAll(user);
    await user.click(screen.getByRole('button', { name: 'Approve and move' }));
    expect(await screen.findByText(/arrived/)).toBeInTheDocument();
  });

  it('asks again about a secret once it is put elsewhere', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await toReview(user);
    await bringAll(user);
    await user.click(screen.getByRole('button', { name: /See every change/ }));
    const [remap] = screen.getAllByRole('button', { name: 'Put elsewhere…' });
    if (remap === undefined) throw new Error('nothing to put elsewhere in the fixture');
    await user.click(remap);
    await user.click(screen.getByRole('button', { name: 'Choose a folder' }));
    await user.click(screen.getByRole('button', { name: 'Use this place' }));
    await user.click(await screen.findByRole('button', { name: 'Done' }));

    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
    });
    expect(screen.getByText('Answer 1 question to continue')).toBeInTheDocument();
  });
});

import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { BridgeEvent } from '@/bindings/BridgeEvent';
import type { CarrovaPort } from '@/engine/types';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../../support/app';

const home = () => screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });

/** `port` with a file manager that sends `paths` (as `send_requested`) on `forward`. */
function fileManager(port: CarrovaPort) {
  const queue: string[][] = [];
  let listener: ((event: BridgeEvent) => void) | null = null;
  const subscribe = port.subscribe.bind(port);
  port.subscribe = (l) => {
    listener = l;
    return subscribe(l);
  };
  port.takeSendRequest = () => Promise.resolve(queue.shift() ?? []);
  return {
    forward: (paths: string[]) => {
      queue.push(paths);
      listener?.({ kind: 'send_requested' });
    },
  };
}

describe('sending over the LAN', () => {
  it('scans, sends the selection, shows the code and ends on Delivered', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const runSend = vi.spyOn(port, 'runSend');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    await user.click(await screen.findByRole('button', { name: 'Get pairing code' }));

    expect(runSend).toHaveBeenCalledOnce();
    expect(await screen.findByText('Your setup is on the new computer.')).toBeInTheDocument();
    expect(screen.getByRole('list', { name: 'Step 4 of 4' })).toBeInTheDocument();
  });

  it('sends every selection edit to the engine', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const select = vi.spyOn(port, 'select');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    await user.click(await screen.findByRole('checkbox', { name: 'Bring Git' }));

    expect(select).toHaveBeenCalledOnce();
    const sent = select.mock.calls[0]?.[1];
    expect(Object.values(sent?.items ?? {}).some((c) => !c.included)).toBe(true);
  });

  it('brings a secret only after its own confirm', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const select = vi.spyOn(port, 'select');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    await user.click(
      await screen.findByRole('button', { name: 'Include SSH key pairs (private keys)…' }),
    );
    const dialog = screen.getByRole('alertdialog', { name: 'Bring SSH key pairs (private keys)?' });
    await user.click(within(dialog).getByRole('button', { name: 'Leave behind' }));
    expect(select).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Include SSH key pairs (private keys)…' }));
    await user.click(screen.getByRole('button', { name: 'Bring them' }));
    expect(select.mock.calls[0]?.[2]).toContainEqual({
      kind: 'confirm_sensitive',
      item: 'ssh/keys',
      confirmed: true,
    });
    expect(
      await screen.findByRole('checkbox', {
        name: 'Bring SSH key pairs (private keys)',
        checked: true,
      }),
    ).toBeInTheDocument();
  });

  it('asks to allow a plugin that changes files before they leave, then sends', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, scenario: 'plugin' });
    const select = vi.spyOn(port, 'select');
    const runSend = vi.spyOn(port, 'runSend');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    const go = await screen.findByRole('button', { name: 'Get pairing code' });
    expect(go).toBeDisabled();

    await user.click(screen.getByRole('button', { name: 'Review… (path-rewrite plugin)' }));
    const dialog = screen.getByRole('dialog', { name: 'Allow “path-rewrite”?' });
    expect(within(dialog).getByText('It cannot use the network.')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Allow for this move' }));

    expect(select.mock.calls.at(-1)?.[2]).toEqual([
      { kind: 'allow_plugin', plugin: 'path-rewrite', allow: true },
    ]);
    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: 'Get pairing code' })).toBeEnabled();
    });
    await user.click(screen.getByRole('button', { name: 'Get pairing code' }));
    expect(runSend).toHaveBeenCalledOnce();
  });

  it("leaves a plugin's items behind when the user doesn't allow it", async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, scenario: 'plugin' });
    const select = vi.spyOn(port, 'select');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    await user.click(await screen.findByRole('button', { name: 'Review… (path-rewrite plugin)' }));
    await user.click(screen.getByRole('button', { name: "Don't allow" }));

    expect(select.mock.calls.at(-1)?.[2]).toEqual([
      { kind: 'allow_plugin', plugin: 'path-rewrite', allow: false },
      { kind: 'item', item: 'starship/config', included: false },
    ]);
    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: 'Get pairing code' })).toBeEnabled();
    });
  });

  it('keeps the selection when saving to a file instead', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const scan = vi.spyOn(port, 'scan');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    await user.click(await screen.findByRole('button', { name: 'Save to a file instead' }));
    await user.click(await screen.findByRole('button', { name: 'Continue' }));

    expect(await screen.findByRole('button', { name: 'Save file' })).toBeInTheDocument();
    expect(scan).toHaveBeenCalledOnce();
  });

  it('asks before showing a new code after a wrong one, and goes on when the user agrees', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, scenario: 'wrong-code' });
    const answer = vi.spyOn(port, 'answerPairing');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    await user.click(await screen.findByRole('button', { name: 'Get pairing code' }));

    const dialog = await screen.findByRole('alertdialog', { name: 'Someone typed a wrong code' });
    expect(within(dialog).getByRole('button', { name: 'Stop pairing' })).toHaveFocus();
    await user.click(within(dialog).getByRole('button', { name: 'Show a new code' }));

    expect(answer).toHaveBeenCalledWith(expect.any(String), true);
    expect(await screen.findByText('Your setup is on the new computer.')).toBeInTheDocument();
  });

  it('stops pairing for good when the user declines a new code', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true, scenario: 'wrong-code' });
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    await user.click(await screen.findByRole('button', { name: 'Get pairing code' }));
    await user.click(await screen.findByRole('button', { name: 'Stop pairing' }));

    expect(await screen.findByRole('heading', { name: 'Pairing stopped' })).toBeInTheDocument();
  });

  it('cancels a running send when the flow is closed', async () => {
    const user = userEvent.setup();
    // Once the code is up, the peer never types it: the send stays running
    // however slowly the test machine gets to the close button.
    let peerAway = false;
    const sleep = () => (peerAway ? new Promise<void>(() => undefined) : Promise.resolve());
    const { port } = await renderApp({ returning: true, sleep });
    const cancel = vi.spyOn(port, 'cancel');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    const getCode = await screen.findByRole('button', { name: 'Get pairing code' });
    peerAway = true;
    await user.click(getCode);
    await screen.findByRole('img', { name: /Pairing code/ });
    await user.click(within(screen.getByRole('banner')).getByRole('button', { name: 'Back' }));

    expect(cancel).toHaveBeenCalledOnce();
    expect(await home()).toBeInTheDocument();
  });
});

describe('Manual sends from settings and the file manager', () => {
  it('start in manual setting opens send in manual', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, settings: { start_in_manual: true } });
    const scan = vi.spyOn(port, 'scan');
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Pick files and folders' }),
    ).toBeInTheDocument();
    expect(scan).not.toHaveBeenCalled();
  });

  it('forwarded paths open manual send with picks', async () => {
    await renderApp({ returning: true, scenario: 'forwarded' });
    const picks = await screen.findByRole('list', { name: 'Your picks' });
    expect(await within(picks).findByText('~/projects/site')).toBeInTheDocument();
    expect(await within(picks).findByText('~/notes.txt')).toBeInTheDocument();
  });

  it('adds forwarded paths to an open send, in Manual', async () => {
    const user = userEvent.setup();
    const port = createFakePort({ stepMs: 0 });
    const files = fileManager(port);
    await renderApp({ returning: true, port });
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    await screen.findByRole('button', { name: 'Get pairing code' });
    files.forward(['/home/ana/notes.txt']);
    const picks = await screen.findByRole('list', { name: 'Your picks' });
    expect(await within(picks).findByText('~/notes.txt')).toBeInTheDocument();
  });

  it('forwarded_paths_refused_while_sending', async () => {
    const user = userEvent.setup();
    let peerAway = false;
    const sleep = () => (peerAway ? new Promise<void>(() => undefined) : Promise.resolve());
    const port = createFakePort({ stepMs: 0, sleep });
    const files = fileManager(port);
    const pick = vi.spyOn(port, 'pick');
    await renderApp({ returning: true, port });
    await home();
    await user.click(screen.getByRole('button', { name: /^Send from this computer/ }));
    const getCode = await screen.findByRole('button', { name: 'Get pairing code' });
    peerAway = true;
    await user.click(getCode);
    await screen.findByRole('img', { name: /Pairing code/ });

    files.forward(['/home/ana/notes.txt']);
    expect(await screen.findByText('Finish or cancel the current move first')).toBeInTheDocument();
    expect(screen.getByRole('img', { name: /Pairing code/ })).toBeInTheDocument();
    expect(pick).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(screen.queryByText('Finish or cancel the current move first')).not.toBeInTheDocument();
  });
});

import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../../support/render';
import type { CallLogView } from '@/bindings/CallLogView';
import type { SimChoice } from '@/bindings/SimChoice';
import { CallsScreen, type CallsModel } from '@/screens/phone/calls/CallsScreen';

const now = Date.now();
const log: CallLogView = {
  groups: [
    {
      days_ago: 0,
      rows: [
        {
          id: 2,
          caller: { number: '+15550101', name: 'Sam Rivera' },
          when_ms: now,
          duration_s: 0,
          kind: 'missed',
        },
      ],
    },
    {
      days_ago: 1,
      rows: [
        {
          id: 1,
          caller: { number: '+15550142', name: null },
          when_ms: now - 86_400_000,
          duration_s: 150,
          kind: 'outgoing',
        },
      ],
    },
  ],
  next: null,
};

const model = (over: Partial<CallsModel> = {}): CallsModel => ({
  phone: 'Pixel 8',
  on: true,
  log,
  failed: false,
  sims: [],
  contacts: [],
  ...over,
});
const sims: SimChoice[] = [
  { id: 3, slot: 1, name: 'Jio', default: false },
  { id: 7, slot: 2, name: 'Airtel', default: true },
];
const intents = () => ({ onDial: vi.fn(), onTurnOn: vi.fn(), onMore: vi.fn(), onSearch: vi.fn() });

describe('Calls search', () => {
  it('filters the log by name or part of a number as you type', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<CallsScreen model={model()} on={on} />);
    const field = screen.getByLabelText('Call a name or number');
    await user.type(field, 'sam');
    expect(on.onSearch).toHaveBeenLastCalledWith('sam');
    expect(screen.getByText('Sam Rivera')).toBeInTheDocument();
    expect(screen.queryByText('+15550142')).not.toBeInTheDocument();
    await user.clear(field);
    await user.type(field, '555-0142');
    expect(screen.queryByText('Sam Rivera')).not.toBeInTheDocument();
    expect(screen.getByText('+15550142')).toBeInTheDocument();
    // A name typed: nothing to dial, only a contact to pick.
    await user.clear(field);
    await user.type(field, 'hanu');
    expect(screen.getByRole('button', { name: 'Call' })).toBeDisabled();
    await user.clear(field);
    expect(screen.getByText('Sam Rivera')).toBeInTheDocument();
    expect(screen.getByText('+15550142')).toBeInTheDocument();
  });

  it("suggests the phone's contacts and calls one from the SIM picked", async () => {
    const user = userEvent.setup();
    const on = intents();
    const contacts = [{ name: 'Hanuman', number: '+91 98765-43210' }];
    renderScreen(<CallsScreen model={model({ sims, contacts })} on={on} />);
    await user.click(screen.getByRole('radio', { name: 'SIM 1 · Jio' }));
    await user.click(screen.getByRole('button', { name: 'Call Hanuman' }));
    expect(on.onDial).toHaveBeenCalledWith('+919876543210', 3);
  });
});

describe('Calls', () => {
  it('lists calls by day and calls back from the phone', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<CallsScreen model={model()} on={on} />);
    expect(screen.getByRole('heading', { name: 'Today' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Yesterday' })).toBeInTheDocument();
    expect(screen.getByText('Outgoing · 3 min')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Call back +15550142' }));
    expect(on.onDial).toHaveBeenCalledWith('+15550142', null);
  });

  it('dials a typed number', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<CallsScreen model={model()} on={on} />);
    expect(screen.getByRole('button', { name: 'Call' })).toBeDisabled();
    await user.type(screen.getByLabelText('Call a name or number'), '555 0199{Enter}');
    // What a dial takes: the spaces go.
    expect(on.onDial).toHaveBeenCalledWith('5550199', null);
    expect(screen.queryByRole('radiogroup', { name: 'Call from' })).not.toBeInTheDocument();
  });

  it('calls from the default SIM, or the one picked, on a dual-SIM phone', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<CallsScreen model={model({ sims })} on={on} />);
    expect(screen.getByRole('radio', { name: 'SIM 2 · Airtel' })).toBeChecked();
    await user.click(screen.getByRole('button', { name: 'Call back +15550142' }));
    expect(on.onDial).toHaveBeenLastCalledWith('+15550142', 7);
    await user.click(screen.getByRole('radio', { name: 'SIM 1 · Jio' }));
    await user.type(screen.getByLabelText('Call a name or number'), '555 0199{Enter}');
    expect(on.onDial).toHaveBeenLastCalledWith('5550199', 3);
  });

  it('says when there are no calls yet', () => {
    renderScreen(<CallsScreen model={model({ log: { groups: [], next: null } })} on={intents()} />);
    expect(screen.getByRole('heading', { name: 'No calls yet' })).toBeInTheDocument();
  });

  it('offers to turn calls on while they are off', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<CallsScreen model={model({ on: false, log: null })} on={on} />);
    await user.click(screen.getByRole('button', { name: 'Turn on calls' }));
    expect(on.onTurnOn).toHaveBeenCalledOnce();
  });
});

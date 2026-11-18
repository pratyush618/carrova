import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Contact } from '@/bindings/Contact';
import { CONTACTS_DEBOUNCE_MS, usePhoneContacts } from '@/flows/calls/usePhoneContacts';

const hanu: Contact = { name: 'Hanuman', number: '+91 98765 43210' };

afterEach(() => {
  vi.useRealTimers();
});

describe('usePhoneContacts', () => {
  it('asks the phone once typing pauses, with the latest text', async () => {
    vi.useFakeTimers();
    const phoneContacts = vi.fn(() => Promise.resolve([hanu]));
    const { result, rerender } = renderHook(
      ({ query }) => usePhoneContacts({ phoneContacts }, 'p8', query, true),
      { initialProps: { query: 'ha' } },
    );
    rerender({ query: ' hanu ' });
    await act(() => vi.advanceTimersByTimeAsync(CONTACTS_DEBOUNCE_MS));
    expect(phoneContacts).toHaveBeenCalledTimes(1);
    expect(phoneContacts).toHaveBeenCalledWith('p8', 'hanu');
    expect(result.current).toEqual([hanu]);
    rerender({ query: '' });
    expect(result.current).toEqual([]);
  });

  it('suggests none from a phone that cannot say, or while off', async () => {
    vi.useFakeTimers();
    const failing = vi.fn(() => Promise.reject(new Error('old phone')));
    const { result } = renderHook(() =>
      usePhoneContacts({ phoneContacts: failing }, 'p8', 'ha', true),
    );
    await act(() => vi.advanceTimersByTimeAsync(CONTACTS_DEBOUNCE_MS));
    expect(failing).toHaveBeenCalled();
    expect(result.current).toEqual([]);
    const off = vi.fn(() => Promise.resolve([hanu]));
    renderHook(() => usePhoneContacts({ phoneContacts: off }, 'p8', 'ha', false));
    await act(() => vi.advanceTimersByTimeAsync(CONTACTS_DEBOUNCE_MS));
    expect(off).not.toHaveBeenCalled();
  });
});

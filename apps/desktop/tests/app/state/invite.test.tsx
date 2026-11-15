import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { EngineStateProvider, useInvite } from '@/app/state/EngineState';
import { PortProvider } from '@/engine/port';
import { createFakePort } from '@/ipc/fake';

describe('the pairing invite', () => {
  it('is held only until its dialog drops it', async () => {
    const port = createFakePort({ stepMs: 0 });
    const wrapper = ({ children }: { readonly children: ReactNode }) => (
      <PortProvider port={port}>
        <EngineStateProvider>{children}</EngineStateProvider>
      </PortProvider>
    );
    const hook = renderHook(() => useInvite(), { wrapper });
    // Let the provider subscribe before the invite goes out.
    await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
    await act(() => port.pairInvite());
    await waitFor(() => {
      expect(hook.result.current.invite?.view.code.code).toBe('4-maple-orbit');
    });
    act(() => {
      hook.result.current.clear();
    });
    expect(hook.result.current.invite).toBeNull();
  });
});

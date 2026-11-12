import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { EngineStateProvider } from '@/app/state/EngineState';
import { PortProvider } from '@/engine/port';
import { useInbox } from '@/flows/inbox/useInbox';
import { createFakePort } from '@/ipc/fake';

describe('useInbox', () => {
  it('reloads the notices as stored when marking them read fails', async () => {
    const port = createFakePort({ stepMs: 0 });
    vi.spyOn(port, 'markNoticesRead').mockRejectedValue({
      code: 'io',
      message: 'disk full',
      hint: null,
    });
    const notices = vi.spyOn(port, 'notices');
    const wrapper = ({ children }: { readonly children: ReactNode }) => (
      <PortProvider port={port}>
        <EngineStateProvider>{children}</EngineStateProvider>
      </PortProvider>
    );
    const inbox = renderHook(() => useInbox(port), { wrapper });
    await waitFor(() => {
      expect(notices).toHaveBeenCalledOnce();
    });

    act(() => {
      inbox.result.current.markRead('all');
    });

    await waitFor(() => {
      expect(notices).toHaveBeenCalledTimes(2);
    });
  });
});

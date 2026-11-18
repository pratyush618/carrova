import { render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { View } from '@/app/nav/navMachine';
import { PhoneNotePopups } from '@/app/shell/PhoneNotePopups';
import type { NoteAlert } from '@/app/state/link';
import { HostProvider } from '@/engine/host';
import { pixel } from '@/screens/devices/fixtures';

const note: NoteAlert = { device: 'p8', key: 'k', locked: false, smsApp: true, n: 1 };

vi.mock('@/app/state/EngineState', () => ({ useLink: () => ({ note }) }));
vi.mock('@/app/controllers/Controllers', () => ({
  useControllers: () => ({
    linked: {
      devices: [
        {
          ...pixel,
          features: [
            { feature: 'messages', group: 'quiet', local: 'on', remote: 'on', effective: 'on' },
          ],
        },
      ],
    },
  }),
}));

function popups(view: View) {
  const showNotePopup = vi.fn(() => Promise.resolve());
  render(
    <HostProvider host={{ showNotePopup }}>
      <PhoneNotePopups view={view} />
    </HostProvider>,
  );
  return showNotePopup;
}

describe('PhoneNotePopups', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('pops up no text while its phone’s Messages is in front', () => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    expect(popups({ kind: 'hub', page: 'messages' })).not.toHaveBeenCalled();
  });

  it('pops a text up on any other page', () => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    expect(popups({ kind: 'hub', page: 'home' })).toHaveBeenCalledOnce();
  });

  it('pops a text up while the window is in the background', () => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(false);
    expect(popups({ kind: 'hub', page: 'messages' })).toHaveBeenCalledOnce();
  });
});

import { useMemo } from 'react';
import type { SessionRecord } from '../../../engine/types';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';

/** Starting points shared by Home, History, sheets and dialogs: one place per intent. */
export function useStartFlows() {
  const { send, save, receive, undo } = useControllers();
  const nav = useNav();
  return useMemo(
    () => ({
      send: () => {
        send.start();
        nav.go({ type: 'startFlow', flow: 'send' });
      },
      receive: () => {
        receive.start('lan');
        nav.go({ type: 'startFlow', flow: 'receive' });
      },
      save: () => {
        save.start();
        nav.go({ type: 'startFlow', flow: 'save' });
      },
      open: () => {
        receive.start('file');
        nav.go({ type: 'startFlow', flow: 'open' });
      },
      /** Pick an interrupted session up again, in the flow it belongs to. */
      resume: (record: SessionRecord) => {
        const file = record.source?.kind === 'archive';
        if (record.role === 'export') {
          send.resume(record.id);
          nav.go({ type: 'startFlow', flow: 'send' });
        } else {
          receive.resume(record.id, file ? 'file' : 'lan');
          nav.go({ type: 'startFlow', flow: file ? 'open' : 'receive' });
        }
      },
      /** Ask before undoing `record`; the engine previews what undo would do. */
      askUndo: (record: SessionRecord) => {
        undo.ask(record.id, new Date(record.updated_at * 1000));
        nav.open({ kind: 'undo-confirm' });
      },
    }),
    [send, save, receive, undo, nav],
  );
}

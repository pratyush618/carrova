import { useCallback, useMemo, useRef, useState } from 'react';
import type { CarrovaPort, RollbackPreview, RollbackReport, SessionId } from '../../engine/types';
import type { UndoPreviewModel } from '../../screens/undo/model';
import type { Remote } from '../remote';
import { useCommand } from '../useRemote';
import { undoPreviewModel } from './undoViews';

export interface UndoController {
  /** The move Undo would reverse, with its preview; `null` when nothing is asked. */
  readonly target: { readonly session: SessionId; readonly preview: UndoPreviewModel } | null;
  readonly rollback: Remote<RollbackReport>;
  /** Ask about undoing `session` (applied at `pointAt`); the engine previews it. */
  readonly ask: (session: SessionId, pointAt: Date) => void;
  readonly confirm: () => void;
}

interface Asked {
  readonly session: SessionId;
  readonly pointAt: Date;
  readonly preview: RollbackPreview | null;
}

/** Undo: the engine's preview first, then `rollback`, then what happened. */
export function useUndo(port: CarrovaPort, onDone: () => void): UndoController {
  const [asked, setAsked] = useState<Asked | null>(null);
  const latest = useRef<SessionId | null>(null);
  const rollback = useCommand(useCallback((id: SessionId) => port.rollback(id), [port]));
  const { run, reset } = rollback;
  const ask = useCallback(
    (session: SessionId, pointAt: Date) => {
      reset();
      latest.current = session;
      setAsked({ session, pointAt, preview: null });
      void port.rollbackPreview(session).then(
        (preview) => {
          if (latest.current === session) setAsked({ session, pointAt, preview });
        },
        // Without a preview the dialog still asks; undo reports what it did.
        () => undefined,
      );
    },
    [port, reset],
  );
  const target = useMemo(
    () =>
      asked === null
        ? null
        : {
            session: asked.session,
            preview: undoPreviewModel(asked.pointAt, new Date(), asked.preview),
          },
    [asked],
  );
  const confirm = useCallback(() => {
    if (asked !== null) void run(asked.session).then(onDone);
  }, [run, asked, onDone]);
  return useMemo(
    () => ({ target, rollback: rollback.state, ask, confirm }),
    [target, rollback.state, ask, confirm],
  );
}

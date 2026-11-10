import { useCallback, useEffect, useMemo, useReducer, useRef } from 'react';
import { toBridgeError } from '../../engine/errors';
import type { DesktopHost } from '../../engine/host';
import type { NearbyPeer } from '../../bindings/NearbyPeer';
import type { PlanEdit } from '../../bindings/PlanEdit';
import type { PlanOverride } from '../../bindings/PlanOverride';
import type {
  ConflictRule,
  ItemKey,
  MoveDefaults,
  CarrovaPort,
  PlanReview,
  Remap,
  SessionId,
} from '../../engine/types';
import type { DecisionAnswer, ImportChoices } from '../../screens/receive/model';
import { current } from '../remote';
import { connectTarget } from './entry';
import { answerEdits, choiceEdits } from './planOptions';
import { initialReceiveFlow, receiveFlowReducer, type ReceiveFlowState } from './receiveFlow';

/** Commands of the receiving side (LAN and file); screens call these, never the port. */
export interface ReceiveController {
  readonly state: ReceiveFlowState;
  readonly start: (origin: 'lan' | 'file') => void;
  readonly setCode: (value: string) => void;
  readonly setAddress: (value: string) => void;
  readonly setPath: (value: string) => void;
  readonly setPassphrase: (value: string) => void;
  /** Present only when the host has a native "open" dialog. */
  readonly choosePath: (() => void) | null;
  /** Connect (LAN) or open the file, then plan with the suggested options. */
  readonly open: () => void;
  readonly answer: (key: ItemKey, answer: DecisionAnswer) => void;
  readonly setRule: (item: ItemKey, rule: ConflictRule) => void;
  readonly setFileRule: (path: string, rule: ConflictRule | null) => void;
  readonly setRemap: (item: ItemKey, remap: Remap | null) => void;
  readonly setChoices: (patch: Partial<ImportChoices>) => void;
  readonly allowCrossOs: (item: ItemKey) => void;
  readonly allowPlugin: (plugin: string) => void;
  /** Don't allow a plugin: the items it would change are left behind. */
  readonly leavePlugin: (plugin: string) => void;
  /** An override the user confirmed in its warning dialog. */
  readonly override: (override: PlanOverride, app: string | null) => void;
  readonly approve: () => void;
  /** Hold (`true`) or continue the running LAN transfer between batches. */
  readonly setPaused: (paused: boolean) => void;
  /** Continue a transfer paused for low space, ignoring the check (after the warning). */
  readonly continueDespiteSpace: () => void;
  /** Reconnect a lost LAN transfer now instead of after the back-off. */
  readonly tryNow: () => void;
  /** Senders nearby offering a code (reassurance only: the code is still typed). */
  readonly browseNearby: () => Promise<NearbyPeer[]>;
  /** Pick the session up again after a failure (what arrived is kept). */
  readonly retry: () => void;
  /** "Cancel, change nothing": ends the review as a dry run. */
  readonly endReview: () => void;
  /** Leave: stops pairing or a running transfer (between steps) and forgets the state. */
  readonly abandon: () => void;
  /** Continue an interrupted import from History. */
  readonly resume: (session: SessionId, origin: 'lan' | 'file') => void;
}

/** `defaults`: the user's suggested defaults (Settings); every new move starts from them. */
export function useReceiveFlow(
  port: CarrovaPort,
  host: DesktopHost,
  defaults: MoveDefaults,
): ReceiveController {
  const [state, dispatch] = useReducer(receiveFlowReducer, initialReceiveFlow);
  const ticket = useRef(0);
  const { session, options } = state;
  const applying = state.apply.status !== 'idle';
  const review = current(state.review);

  // The first plan of a session; the user's changes then go to the engine as edits.
  useEffect(() => {
    if (session === null || applying) return;
    const mine = ++ticket.current;
    dispatch({ type: 'plan', action: { type: 'start' } });
    port.plan(session, options).then(
      (value) => {
        if (mine === ticket.current) dispatch({ type: 'plan', action: { type: 'ok', value } });
      },
      (reason: unknown) => {
        if (mine === ticket.current) {
          dispatch({ type: 'plan', action: { type: 'fail', error: toBridgeError(reason) } });
        }
      },
    );
  }, [port, session, options, applying]);

  /** Edits in the order the user made them, each sent once the one before is answered. */
  const edits = useRef<Promise<unknown>>(Promise.resolve());
  const edit = useCallback(
    (changes: PlanEdit[]) => {
      if (session === null || changes.length === 0) return;
      const mine = ++ticket.current;
      dispatch({ type: 'plan', action: { type: 'start' } });
      const reply: Promise<PlanReview> = edits.current.then(() => port.editPlan(session, changes));
      edits.current = reply.catch(() => undefined);
      reply.then(
        (value) => {
          if (mine === ticket.current) dispatch({ type: 'plan', action: { type: 'ok', value } });
        },
        (reason: unknown) => {
          if (mine === ticket.current) {
            dispatch({ type: 'plan', action: { type: 'fail', error: toBridgeError(reason) } });
          }
        },
      );
    },
    [port, session],
  );

  const browseNearby = useCallback(() => port.browseNearby(), [port]);

  const opened = useCallback((task: Promise<{ session: SessionId }>) => {
    const mine = ++ticket.current;
    dispatch({ type: 'opening' });
    task.then(
      (result) => {
        if (mine === ticket.current) dispatch({ type: 'opened', session: result.session });
      },
      (reason: unknown) => {
        if (mine === ticket.current) dispatch({ type: 'openFailed', error: toBridgeError(reason) });
      },
    );
  }, []);

  /** Pick session `id` up again and plan from the options it last planned with. */
  const resumed = useCallback(
    (id: SessionId) => {
      const mine = ++ticket.current;
      dispatch({ type: 'opening' });
      port
        .resume(id)
        .then(async ({ session: back }) => ({ back, stored: await port.planOptions(back) }))
        .then(
          ({ back, stored }) => {
            if (mine === ticket.current) {
              dispatch({ type: 'resumed', session: back, options: stored });
            }
          },
          (reason: unknown) => {
            if (mine === ticket.current) {
              dispatch({ type: 'openFailed', error: toBridgeError(reason) });
            }
          },
        );
    },
    [port],
  );

  const open = useCallback(() => {
    if (state.origin === 'lan') opened(port.connect(connectTarget(state.code)));
    else opened(port.openArchive(state.file.path.trim(), state.file.passphrase));
  }, [opened, port, state.origin, state.code, state.file.path, state.file.passphrase]);

  const { pickOpenPath } = host;
  const choosePath = useMemo(
    () =>
      pickOpenPath === undefined
        ? null
        : () => {
            void pickOpenPath().then((path) => {
              if (path !== null) dispatch({ type: 'path', value: path });
            });
          },
    [pickOpenPath],
  );

  const approve = useCallback(() => {
    if (session === null) return;
    const mine = ++ticket.current;
    dispatch({ type: 'apply', action: { type: 'start' } });
    // A resumed session's plan approved earlier with the same options needs no second approval.
    const approved = review?.approval === 'given' ? Promise.resolve() : port.approve(session);
    approved
      .then(() => port.apply(session))
      .then(
        (value) => {
          if (mine === ticket.current) dispatch({ type: 'apply', action: { type: 'ok', value } });
        },
        (reason: unknown) => {
          if (mine === ticket.current) {
            dispatch({ type: 'apply', action: { type: 'fail', error: toBridgeError(reason) } });
          }
        },
      );
  }, [port, session, review]);

  const connecting = state.code.status === 'connecting';
  /** Stops whatever runs: pairing (no session yet) or a transfer between steps. */
  const stop = useCallback(() => {
    ticket.current++;
    if (connecting) void port.cancelConnect();
    if (session !== null && state.apply.status === 'loading') void port.cancel(session);
  }, [port, connecting, session, state.apply.status]);

  const start = useCallback(
    (origin: 'lan' | 'file') => {
      ticket.current++;
      dispatch({ type: 'start', origin, defaults });
    },
    [defaults],
  );

  return useMemo(
    () => ({
      state,
      start,
      setCode: (value: string) => {
        dispatch({ type: 'code', value });
      },
      setAddress: (value: string) => {
        dispatch({ type: 'address', value });
      },
      setPath: (value: string) => {
        dispatch({ type: 'path', value });
      },
      setPassphrase: (value: string) => {
        dispatch({ type: 'passphrase', value });
      },
      choosePath,
      open,
      answer: (key: ItemKey, answer: DecisionAnswer) => {
        dispatch({ type: 'answer', key, answer });
        edit(answerEdits(key, answer));
      },
      setRule: (item: ItemKey, rule: ConflictRule) => {
        edit([{ kind: 'item_rule', item, rule }]);
      },
      setFileRule: (path: string, rule: ConflictRule | null) => {
        edit([{ kind: 'path_rule', path, rule }]);
      },
      setRemap: (item: ItemKey, remap: Remap | null) => {
        edit([{ kind: 'remap', item, remap }]);
      },
      setChoices: (patch: Partial<ImportChoices>) => {
        edit(choiceEdits(patch));
      },
      allowCrossOs: (item: ItemKey) => {
        edit([{ kind: 'allow_cross_os', item, allow: true }]);
      },
      allowPlugin: (plugin: string) => {
        edit([{ kind: 'allow_plugin', plugin, allow: true }]);
      },
      leavePlugin: (plugin: string) => {
        const items = state.asked.plugins[plugin]?.items ?? [];
        for (const key of items) dispatch({ type: 'answer', key, answer: 'leave' });
        edit(items.flatMap((key) => answerEdits(key, 'leave')));
      },
      override: (override: PlanOverride, app: string | null) => {
        if (app !== null) dispatch({ type: 'answer', key: app, answer: 'closed' });
        edit([{ kind: 'override', check: override }]);
      },
      approve,
      setPaused: (paused: boolean) => {
        if (session !== null) void port.setTransferPaused(session, paused);
      },
      continueDespiteSpace: () => {
        if (session !== null) void port.ignoreFreeSpace(session);
      },
      tryNow: () => {
        if (session !== null) void port.retryConnection(session);
      },
      browseNearby,
      retry: () => {
        if (session !== null) resumed(session);
      },
      endReview: () => {
        if (session !== null) void port.endReview(session);
        start(state.origin);
      },
      abandon: () => {
        stop();
        start(state.origin);
      },
      resume: (id: SessionId, origin: 'lan' | 'file') => {
        start(origin);
        resumed(id);
      },
    }),
    [state, start, choosePath, open, edit, browseNearby, approve, session, resumed, port, stop],
  );
}

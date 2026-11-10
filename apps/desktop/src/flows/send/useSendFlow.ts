import { useCallback, useMemo, useReducer, useRef } from 'react';
import { useEngineDispatch } from '../../app/state/EngineState';
import { toBridgeError } from '../../engine/errors';
import type { CarrovaPort, ScanResult, SendReport, SessionId } from '../../engine/types';
import { toEdits, type SelectionChange } from './selectionChange';
import { initialSendFlow, sendFlowReducer, type SendFlowState } from './sendFlow';

/** Commands of the sending side; screens call these, never the port. */
export interface SendController {
  readonly state: SendFlowState;
  /** Fresh scan (also forgets any earlier selection). */
  readonly start: () => void;
  readonly edit: (change: SelectionChange) => void;
  readonly resetToSuggested: () => void;
  /** Waits until the engine has seen the latest selection; `true` if it can be used. */
  readonly confirmSelection: () => Promise<boolean>;
  readonly send: () => void;
  /** Stop the current code and start pairing again with a new one. */
  readonly newCode: () => void;
  readonly answerPrompt: (offerNewCode: boolean) => void;
  /** Back to the selection (stops pairing). */
  readonly back: () => void;
  /** Leave the flow: stops whatever runs and forgets the state. */
  readonly abandon: () => void;
  /** Pick up an interrupted send from History. */
  readonly resume: (session: SessionId) => void;
}

const usable = (result: ScanResult | null): boolean =>
  result !== null && result.view.issues.length === 0;

/**
 * The effect layer over {@link sendFlowReducer}: each command is one port call.
 * `includeCaches`: the user's suggested default (Settings) for a fresh scan.
 */
export function useSendFlow(port: CarrovaPort, includeCaches: boolean): SendController {
  const [state, dispatch] = useReducer(sendFlowReducer, initialSendFlow);
  const engine = useEngineDispatch();
  const session = useRef<SessionId | null>(null);
  /**
   * The engine's latest reply for the session, once every queued request has
   * answered. Selection requests run one after another, each on the result of
   * the one before, so the engine applies the user's edits in order.
   */
  const latest = useRef<Promise<ScanResult | null>>(Promise.resolve(null));
  const ticket = useRef(0);
  const running = useRef(false);

  const stop = useCallback(() => {
    ticket.current++;
    if (running.current && session.current !== null) void port.cancel(session.current);
    running.current = false;
  }, [port]);

  /** Queue a selection request on `base`; only the newest one's outcome is shown. */
  const enqueue = useCallback(
    (request: (id: SessionId, base: ScanResult) => Promise<ScanResult>) => {
      const id = session.current;
      if (id === null) return;
      const queued: Promise<ScanResult | null> = latest.current.then(async (base) => {
        if (base === null) return null;
        try {
          const result = await request(id, base);
          if (latest.current === queued) dispatch({ type: 'selected', result });
          return result;
        } catch (reason: unknown) {
          if (latest.current === queued) {
            dispatch({ type: 'selectFailed', error: toBridgeError(reason) });
          }
          // Refused edits change nothing: the engine keeps the previous selection.
          return base;
        }
      });
      latest.current = queued;
    },
    [],
  );

  const edit = useCallback(
    (change: SelectionChange) => {
      enqueue((id, base) => port.select(id, base.selection, toEdits(change, base.selection)));
    },
    [port, enqueue],
  );

  const start = useCallback(() => {
    stop();
    const mine = ticket.current;
    dispatch({ type: 'reset' });
    dispatch({ type: 'scan', action: { type: 'start' } });
    port.scan().then(
      (result) => {
        if (mine !== ticket.current) return;
        session.current = result.session;
        latest.current = Promise.resolve(result);
        dispatch({ type: 'scan', action: { type: 'ok', value: result } });
        // The user's suggested default (Settings): still one tap to undo.
        if (includeCaches) edit({ kind: 'caches', included: true });
      },
      (reason: unknown) => {
        if (mine === ticket.current) {
          dispatch({ type: 'scan', action: { type: 'fail', error: toBridgeError(reason) } });
        }
      },
    );
  }, [port, stop, edit, includeCaches]);

  const resetToSuggested = useCallback(() => {
    enqueue(async (id) => {
      const suggested = await port.suggest(id, 'suggested');
      return port.select(id, suggested.selection, []);
    });
  }, [port, enqueue]);

  const confirmSelection = useCallback(async () => usable(await latest.current), []);

  /** Runs a send task; only the newest one's outcome lands. */
  const track = useCallback((task: Promise<SendReport>) => {
    const mine = ++ticket.current;
    running.current = true;
    dispatch({ type: 'run', action: { type: 'start' } });
    task.then(
      (report) => {
        if (mine !== ticket.current) return;
        running.current = false;
        dispatch({ type: 'run', action: { type: 'ok', value: report } });
      },
      (reason: unknown) => {
        if (mine !== ticket.current) return;
        running.current = false;
        dispatch({ type: 'run', action: { type: 'fail', error: toBridgeError(reason) } });
      },
    );
  }, []);

  const runSend = useCallback(
    (id: SessionId) => {
      track(port.runSend(id));
    },
    [port, track],
  );

  const send = useCallback(() => {
    void (async () => {
      const id = session.current;
      if (id !== null && (await confirmSelection())) runSend(id);
    })();
  }, [confirmSelection, runSend]);

  const newCode = useCallback(() => {
    const id = session.current;
    if (id === null) return;
    stop();
    runSend(id);
  }, [runSend, stop]);

  const answerPrompt = useCallback(
    (offerNewCode: boolean) => {
      const id = session.current;
      if (id === null) return;
      engine({ type: 'pairing_answered', session: id });
      void port.answerPairing(id, offerNewCode);
    },
    [engine, port],
  );

  const back = useCallback(() => {
    stop();
    dispatch({ type: 'back' });
  }, [stop]);

  const abandon = useCallback(() => {
    stop();
    dispatch({ type: 'reset' });
  }, [stop]);

  const resume = useCallback(
    (id: SessionId) => {
      stop();
      session.current = id;
      dispatch({ type: 'resumed', session: id });
      track(
        port.resume(id).then((opened) => {
          session.current = opened.session;
          return port.runSend(opened.session);
        }),
      );
    },
    [port, stop, track],
  );

  return useMemo(
    () => ({
      state,
      start,
      edit,
      resetToSuggested,
      confirmSelection,
      send,
      newCode,
      answerPrompt,
      back,
      abandon,
      resume,
    }),
    [
      state,
      start,
      edit,
      resetToSuggested,
      confirmSelection,
      send,
      newCode,
      answerPrompt,
      back,
      abandon,
      resume,
    ],
  );
}

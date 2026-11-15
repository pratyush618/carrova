import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLink } from '../../app/state/EngineState';
import type {
  BridgeError,
  CarrovaPort,
  Feature,
  FeatureState,
  LinkedDeviceView,
} from '../../engine/types';
import { raise } from '../../ipc/reject';

/** A pairing under way: the device it linked (if found), and how to stop it. */
export interface Pairing {
  /** Rejects with `cancelled` once stopped. */
  readonly done: Promise<LinkedDeviceView | null>;
  /** Stop it; one not started yet never starts. */
  readonly cancel: () => void;
}

const CANCELLED: BridgeError = { code: 'cancelled', message: 'cancelled', hint: null };

export interface LinkedController {
  /** Linked devices (empty until loaded). */
  readonly devices: readonly LinkedDeviceView[];
  /** Load again; resolves with the list now in force (`null` if the load failed). */
  readonly reload: () => Promise<readonly LinkedDeviceView[] | null>;
  /** Change one feature on this computer's side; shown at once, then confirmed by a reload. */
  readonly setFeature: (id: string, feature: Feature, state: FeatureState) => void;
  /** The engine refused the last feature change. */
  readonly failed: boolean;
  /** Invite a phone (the invite arrives as an event). */
  readonly pairDevice: () => Pairing;
  /** Link a computer by its code. */
  readonly linkComputer: (ticket: string) => Pairing;
  readonly ring: (id: string) => Promise<void>;
  readonly sendClipboard: (id: string) => Promise<void>;
  readonly sendFiles: (id: string, paths: string[]) => Promise<void>;
}

/** `device` with `feature` set to `state` here (the gate is the engine's: it shows on reload). */
const withLocal = (
  device: LinkedDeviceView,
  feature: Feature,
  state: FeatureState,
): LinkedDeviceView => ({
  ...device,
  features: device.features.map((f) => (f.feature === feature ? { ...f, local: state } : f)),
});

/**
 * Linked devices: loaded on mount and whenever a link or its policy changes
 * (`link_up`, `link_down`, `policy_changed`). Only the latest load lands.
 */
export function useLinkedDevices(port: CarrovaPort): LinkedController {
  const [devices, setDevices] = useState<readonly LinkedDeviceView[]>([]);
  const [failed, setFailed] = useState(false);
  const latest = useRef(0);
  const { changes } = useLink();

  const reload = useCallback(() => {
    const ticket = ++latest.current;
    return port.linkedDevices().then(
      (list) => {
        if (ticket === latest.current) setDevices(list);
        return list;
      },
      () => null,
    );
  }, [port]);

  useEffect(() => {
    void reload();
  }, [reload, changes]);

  const setFeature = useCallback(
    (id: string, feature: Feature, state: FeatureState) => {
      setFailed(false);
      setDevices((list) => list.map((d) => (d.id === id ? withLocal(d, feature, state) : d)));
      void port
        .setFeature(id, feature, state)
        .catch(() => {
          setFailed(true);
        })
        .finally(() => void reload());
    },
    [port, reload],
  );

  // Each device's state before a pairing, to tell which one it linked.
  const before = useRef<ReadonlyMap<string, LinkedDeviceView['state']>>(new Map());
  useEffect(() => {
    before.current = new Map(devices.map((d) => [d.id, d.state]));
  }, [devices]);
  const linkedBy = useCallback(
    async (pairing: Promise<void>) => {
      const was = before.current;
      await pairing;
      const list = await reload();
      // A new device, else a known one that was not linked before (linked again).
      return (
        list?.find((d) => !was.has(d.id)) ??
        list?.find((d) => d.state === 'linked' && was.get(d.id) !== 'linked') ??
        null
      );
    },
    [reload],
  );
  // The engine pairs one at a time and refuses a new one until the last has ended.
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const startPairing = useCallback(
    (start: () => Promise<void>): Pairing => {
      let cancelled = false;
      let started = false;
      let ended = false;
      const run = queue.current.then(() => {
        if (cancelled) raise(CANCELLED);
        started = true;
        return start();
      });
      queue.current = run.then(
        () => (ended = true),
        () => (ended = true),
      );
      return {
        done: linkedBy(run),
        cancel: () => {
          if (cancelled) return;
          cancelled = true;
          if (started && !ended) void port.cancelConnect().catch(() => undefined);
        },
      };
    },
    [port, linkedBy],
  );
  const pairDevice = useCallback(() => startPairing(() => port.pairInvite()), [port, startPairing]);
  const linkComputer = useCallback(
    (ticket: string) => startPairing(() => port.linkComputer(ticket.trim())),
    [port, startPairing],
  );
  const ring = useCallback((id: string) => port.ring(id), [port]);
  const sendClipboard = useCallback((id: string) => port.sendClipboard(id), [port]);
  const sendFiles = useCallback((id: string, paths: string[]) => port.sendFiles(id, paths), [port]);

  return useMemo(
    () => ({
      devices,
      reload,
      setFeature,
      failed,
      pairDevice,
      linkComputer,
      ring,
      sendClipboard,
      sendFiles,
    }),
    [devices, reload, setFeature, failed, pairDevice, linkComputer, ring, sendClipboard, sendFiles],
  );
}

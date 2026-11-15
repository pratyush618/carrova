import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLink } from '../../app/state/EngineState';
import type { CarrovaPort, Feature, FeatureState, LinkedDeviceView } from '../../engine/types';

export interface LinkedController {
  /** Linked devices (empty until loaded). */
  readonly devices: readonly LinkedDeviceView[];
  /** Load again; resolves with the list now in force (`null` if the load failed). */
  readonly reload: () => Promise<readonly LinkedDeviceView[] | null>;
  /** Change one feature on this computer's side; shown at once, then confirmed by a reload. */
  readonly setFeature: (id: string, feature: Feature, state: FeatureState) => void;
  /** The engine refused the last feature change. */
  readonly failed: boolean;
  /** Invite a phone (the invite arrives as an event); resolves with the device that linked, if found. */
  readonly pairPhone: () => Promise<LinkedDeviceView | null>;
  /** Link a computer by its code; resolves with the device that linked, if found. */
  readonly linkComputer: (ticket: string) => Promise<LinkedDeviceView | null>;
  /** Stop a pairing still waiting. */
  readonly cancelPairing: () => void;
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

  // The ids before a pairing, to tell which device it linked.
  const ids = useRef<ReadonlySet<string>>(new Set());
  useEffect(() => {
    ids.current = new Set(devices.map((d) => d.id));
  }, [devices]);
  const linkedBy = useCallback(
    async (pairing: Promise<void>) => {
      const before = ids.current;
      await pairing;
      const list = await reload();
      return list?.find((d) => !before.has(d.id)) ?? null;
    },
    [reload],
  );
  const pairPhone = useCallback(() => linkedBy(port.pairInvite()), [port, linkedBy]);
  const linkComputer = useCallback(
    (ticket: string) => linkedBy(port.linkComputer(ticket.trim())),
    [port, linkedBy],
  );
  const cancelPairing = useCallback(() => {
    void port.cancelConnect().catch(() => undefined);
  }, [port]);
  const ring = useCallback((id: string) => port.ring(id), [port]);
  const sendClipboard = useCallback((id: string) => port.sendClipboard(id), [port]);
  const sendFiles = useCallback((id: string, paths: string[]) => port.sendFiles(id, paths), [port]);

  return useMemo(
    () => ({
      devices,
      reload,
      setFeature,
      failed,
      pairPhone,
      linkComputer,
      cancelPairing,
      ring,
      sendClipboard,
      sendFiles,
    }),
    [
      devices,
      reload,
      setFeature,
      failed,
      pairPhone,
      linkComputer,
      cancelPairing,
      ring,
      sendClipboard,
      sendFiles,
    ],
  );
}

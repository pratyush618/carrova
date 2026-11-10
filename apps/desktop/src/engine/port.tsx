import { createContext, useContext, type ReactNode } from 'react';
import type { CarrovaPort } from './types';

const PortContext = createContext<CarrovaPort | null>(null);

/**
 * Hands the engine port to everything below. The app gets the Tauri adapter
 * or the in-memory fake from `../ipc`; tests pass their own double. Nothing
 * else ever imports an adapter.
 */
export function PortProvider({
  port,
  children,
}: {
  readonly port: CarrovaPort;
  readonly children: ReactNode;
}) {
  return <PortContext value={port}>{children}</PortContext>;
}

/** The engine port. Throws outside {@link PortProvider} (a wiring bug). */
export function usePort(): CarrovaPort {
  const port = useContext(PortContext);
  if (port === null) throw new Error('usePort must be used inside PortProvider');
  return port;
}

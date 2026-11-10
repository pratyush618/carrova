import { useCallback, useMemo } from 'react';
import type { EngineCatalogEntry, CarrovaPort, RecipesStatus } from '../../engine/types';
import { current } from '../remote';
import { useRemote } from '../useRemote';

export interface CatalogController {
  readonly entries: readonly EngineCatalogEntry[] | null;
  readonly status: RecipesStatus | null;
  readonly reload: () => void;
}

/** "Apps Carrova knows": the catalogue and where its recipes came from. */
export function useCatalog(port: CarrovaPort): CatalogController {
  const catalog = useRemote(useCallback(() => port.catalog(), [port]));
  const status = useRemote(useCallback(() => port.recipesStatus(), [port]));
  const reloadCatalog = catalog.run;
  const reloadStatus = status.run;
  const reload = useCallback(() => {
    void reloadCatalog();
    void reloadStatus();
  }, [reloadCatalog, reloadStatus]);
  return useMemo(
    () => ({ entries: current(catalog.state), status: current(status.state), reload }),
    [catalog.state, status.state, reload],
  );
}

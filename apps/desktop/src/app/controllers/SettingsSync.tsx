import { useEffect } from 'react';
import { usePreferences } from '../../preferences/preferences';
import { useControllers } from './Controllers';

/** Applies the saved theme and motion choice to the document; renders nothing. */
export function SettingsSync() {
  const { settings } = useControllers();
  const { update } = usePreferences();
  const theme = settings.settings?.theme ?? null;
  const reduce = settings.settings?.reduce_motion === true;
  useEffect(() => {
    update({ theme: theme ?? 'system', reduceMotion: reduce });
  }, [update, theme, reduce]);
  return null;
}

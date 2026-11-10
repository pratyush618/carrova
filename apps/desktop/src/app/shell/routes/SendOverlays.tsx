import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { current } from '../../../flows/remote';
import { customizeModel, rawFolders, type CustomizeUi } from '../../../flows/send/sendViews';
import { CustomizeSheet } from '../../../screens/send/CustomizeSheet';
import { UnrecognisedSheet } from '../../../screens/send/UnrecognisedSheet';
import type { MinSize } from '../../../screens/send/model';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';

/** Raw folders shown by default and per size filter. */
const MIN_BYTES: Readonly<Record<MinSize, number>> = {
  '10mb': 10_000_000,
  '100mb': 100_000_000,
  any: 0,
};

export function CustomizeOverlay() {
  const { send } = useControllers();
  const nav = useNav();
  const [ui, setUi] = useState<CustomizeUi>({ filter: '', expanded: [] });
  const scan = current(send.state.scan);
  if (scan === null) return null;
  return (
    <CustomizeSheet
      model={customizeModel(scan, ui, MIN_BYTES['10mb'])}
      on={{
        onFilter: (filter) => {
          setUi((u) => ({ ...u, filter }));
        },
        onToggleApp: (app, included) => {
          send.edit({ kind: 'app', app, included });
        },
        onToggleItem: (key, included) => {
          send.edit({ kind: 'item', key, included });
        },
        onTogglePath: (key, path, included) => {
          send.edit({ kind: 'path', key, path, included });
        },
        onExpand: (app) => {
          setUi((u) => ({
            ...u,
            expanded: u.expanded.includes(app)
              ? u.expanded.filter((a) => a !== app)
              : [...u.expanded, app],
          }));
        },
        onIncludeAnyway: (key) => {
          nav.open({ kind: 'override', reason: 'unmovable', subject: key });
        },
        onUnrecognised: () => {
          nav.open({ kind: 'unrecognised' });
        },
        onIncludeCaches: (included) => {
          send.edit({ kind: 'caches', included });
        },
        onReset: send.resetToSuggested,
        onDone: nav.close,
      }}
    />
  );
}

export function UnrecognisedOverlay() {
  const { t } = useTranslation();
  const { send } = useControllers();
  const nav = useNav();
  const [minSize, setMinSize] = useState<MinSize>('10mb');
  const scan = current(send.state.scan);
  if (scan === null) return null;
  return (
    <UnrecognisedSheet
      model={{ minSize, folders: rawFolders(scan, MIN_BYTES[minSize], t) }}
      on={{
        onMinSize: setMinSize,
        onToggle: (key, included) => {
          send.edit({ kind: 'item', key, included });
        },
        onDone: nav.close,
      }}
    />
  );
}

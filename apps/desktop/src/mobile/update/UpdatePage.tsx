import { useEffect, useState } from 'react';
import { useControllers } from '../../app/controllers/Controllers';
import { useHost } from '../../engine/host';
import { usePort } from '../../engine/port';
import { releaseNotesUrl } from './model';
import { PhoneUpdate } from './PhoneUpdate';

/** The update page over the app's update controller: asks Wi-Fi or mobile data when off Wi-Fi. */
export function UpdatePage({ onBack }: { readonly onBack: () => void }) {
  const port = usePort();
  const host = useHost();
  const { update, settings } = useControllers();
  const [offWifi, setOffWifi] = useState(false);
  useEffect(() => {
    port.lanAvailable().then(
      (on) => {
        setOffWifi(!on);
      },
      () => undefined,
    );
  }, [port]);
  const model = update.model;
  return (
    <PhoneUpdate
      model={model}
      current={settings.about?.app_version ?? null}
      askNetwork={offWifi && settings.settings?.app_update.mobile_data !== true}
      on={{
        onBack,
        onDownload: update.install,
        onDownloadOnMobileData: update.installOnMobileData,
        onCancel: update.cancel,
        onPause: update.pause,
        onUseMobileData: update.installAnyway,
        onOpenPermission: update.openPermission,
        onReleaseNotes: () => {
          if ('version' in model && model.version !== null)
            void host.openUrl?.(releaseNotesUrl(model.version));
        },
      }}
    />
  );
}

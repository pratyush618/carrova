import { useTranslation } from 'react-i18next';
import type { PhoneSettings } from '../../bindings/PhoneSettings';
import type { RingHere } from '../../bindings/RingHere';
import { Banner, Button, Row, Segmented, Toggle } from '../../ui';
import { Group } from './Group';
import { ROW_TITLE } from './sections';

const RINGS: readonly RingHere[] = ['always', 'while_active', 'never'];

/** Settings › Calls and Bluetooth: its state and the user's choices. */
export interface CallsSettingsProps {
  /** `null` until loaded. */
  readonly phone: PhoneSettings | null;
  /** The last change could not be made. */
  readonly failed: boolean;
  readonly on: {
    readonly onPhone: (patch: Partial<PhoneSettings>) => void;
    /** Undo the audio setup, then run the wizard again. */
    readonly onAudioAgain: () => void;
    readonly onAudioOff: () => void;
  };
}

/** Where calls ring. */
export function CallsGroup({ calls }: { readonly calls?: CallsSettingsProps | undefined }) {
  const { t } = useTranslation();
  if (calls === undefined) return null;
  const { phone, on } = calls;
  return (
    <>
      {calls.failed && <Banner tone="warning" title={t('calls.settings.failed')} />}
      <Group title={t('calls.settings.group')}>
        <Row
          title={t(ROW_TITLE.callAudio)}
          sub={t('calls.settings.audioSub')}
          trail={
            <>
              <Button size="sm" onClick={on.onAudioOff}>
                {t('calls.settings.off')}
              </Button>
              <Button size="sm" onClick={on.onAudioAgain}>
                {t('calls.settings.again')}
              </Button>
            </>
          }
        />
        {phone !== null && (
          <>
            <Row
              title={t(ROW_TITLE.ringHere)}
              trail={
                <Segmented<RingHere>
                  label={t(ROW_TITLE.ringHere)}
                  options={RINGS.map((value) => ({
                    value,
                    label: t(`calls.settings.rings.${value}`),
                  }))}
                  value={phone.ring_here}
                  onChange={(ring_here) => {
                    on.onPhone({ ring_here });
                  }}
                />
              }
            />
            <Row
              title={t(ROW_TITLE.onlyActive)}
              sub={t('calls.settings.onlyActiveSub')}
              trail={
                <Toggle
                  on={phone.only_while_active}
                  label={t(ROW_TITLE.onlyActive)}
                  onChange={(only_while_active) => {
                    on.onPhone({ only_while_active });
                  }}
                />
              }
            />
          </>
        )}
      </Group>
    </>
  );
}

import { useTranslation } from 'react-i18next';
import type { FeatureState, LinkedDeviceView } from '../../engine/types';
import { AppGlyph, Badge, Button, Icon, LinkRow, Muted, Row } from '../../ui';
import { deviceIcon, FEATURE_ICON, type Phase1Feature } from '../devices/model';
import { Group } from './Group';
import type { SettingsIntents } from './model';
import { ROW_TITLE } from './sections';

/** What a new phone link starts with (the spec's policy table); computers start off. */
const PHONE_DEFAULT: Readonly<Record<Phase1Feature, FeatureState>> = {
  clipboard: 'on',
  ring: 'on',
  files: 'ask',
  handoff: 'ask',
};

const GROUPS = { quiet: ['clipboard', 'ring'], ask: ['files', 'handoff'] } as const;
const GROUP_ORDER = ['quiet', 'ask'] as const;

/**
 * What linked devices can do, and how a new phone starts. Stated, not
 * switched: each device's page is where a feature changes.
 */
export function FeaturesGroup() {
  const { t } = useTranslation();
  return (
    <>
      {GROUP_ORDER.map((group) => (
        <Group key={group} title={t(`ecosystem.groups.${group}`)}>
          {GROUPS[group].map((feature) => (
            <Row
              key={feature}
              lead={<AppGlyph glyph={{ icon: FEATURE_ICON[feature] }} size="sm" />}
              title={t(`ecosystem.features.${feature}.title`)}
              sub={t(`ecosystem.features.${feature}.body`)}
              trail={<Badge>{t(`ecosystem.states.${PHONE_DEFAULT[feature]}`)}</Badge>}
            />
          ))}
        </Group>
      ))}
      <p className="s-settings-sub">{t('ecosystem.settings.featuresFoot')}</p>
    </>
  );
}

/** Each linked device (opens its page), and the two ways to link a new one. */
export function LinkedGroup({
  linked,
  on,
}: {
  readonly linked: readonly LinkedDeviceView[];
  readonly on: SettingsIntents;
}) {
  const { t } = useTranslation();
  return (
    <>
      <Group title={t('ecosystem.settings.linkedGroup')}>
        {linked.length === 0 ? (
          <li className="p-row">
            <Muted>{t('ecosystem.settings.linkedNone')}</Muted>
          </li>
        ) : (
          linked.map((device) => (
            <LinkRow
              key={device.id}
              off={device.state === 'away'}
              lead={<AppGlyph glyph={{ icon: deviceIcon(device.os) }} size="sm" />}
              title={device.name}
              sub={`${t(`settings.about.os.${device.os}`)} · ${t(`ecosystem.status.${device.state}`)}`}
              trail={<Icon name="chev" />}
              onClick={() => {
                on.onOpenDevice(device.id);
              }}
            />
          ))
        )}
      </Group>
      <Group title={t('ecosystem.settings.newGroup')}>
        <Row
          title={t(ROW_TITLE.addDevice)}
          sub={t('ecosystem.settings.addDeviceSub')}
          trail={
            <Button size="sm" icon="phone" onClick={on.onAddDevice}>
              {t('ecosystem.settings.add')}
            </Button>
          }
        />
        <Row
          title={t(ROW_TITLE.linkComputer)}
          sub={t('ecosystem.settings.linkComputerSub')}
          trail={
            <Button size="sm" icon="laptop" onClick={on.onLinkComputer}>
              {t('ecosystem.settings.link')}
            </Button>
          }
        />
      </Group>
    </>
  );
}

/** The history (in a sheet), and the two rules that are stated, not switched. */
export function ClipboardGroup({ on }: { readonly on: SettingsIntents }) {
  const { t } = useTranslation();
  return (
    <Group title={t('ecosystem.settings.clipGroup')}>
      <LinkRow
        title={t(ROW_TITLE.clipHistory)}
        sub={t('ecosystem.settings.historySub')}
        trail={<Icon name="chev" />}
        onClick={on.onClipHistory}
      />
      <Row
        title={t(ROW_TITLE.clipPasswords)}
        sub={t('ecosystem.settings.passwordsSub')}
        trail={<Badge>{t('ecosystem.settings.always')}</Badge>}
      />
      <Row title={t(ROW_TITLE.clipLimits)} sub={t('ecosystem.settings.limitsSub')} />
    </Group>
  );
}

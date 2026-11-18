import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { CallSetupStep } from '../../../bindings/CallSetupStep';
import type { CallsSetup } from '../../../flows/calls/useCallsSetup';
import { Badge, Banner, Button, Check, Dialog, List, Row, type IconName } from '../../../ui';
import './callsSetup.css';

const ICON: Record<CallSetupStep, IconName> = {
  intro: 'bt',
  phone: 'phone',
  install: 'box',
  pair: 'bt',
  test: 'mic',
  done: 'check',
  off: 'bt',
  no_bt: 'warn',
};

/**
 * Calls setup: this computer becomes a Bluetooth headset for the phone, once.
 * The audio change waits for the user's "I agree"; talking on the phone is
 * always the way out.
 */
export function CallsSetupDialog({
  setup,
  onClose,
}: {
  readonly setup: CallsSetup;
  readonly onClose: () => void;
}) {
  const { t } = useTranslation();
  const { step, busy } = setup;
  const btn = (label: string, onClick: () => void, primary = false) => (
    <Button kind={primary ? 'primary' : 'secondary'} disabled={busy} onClick={onClick}>
      {label}
    </Button>
  );
  const toPhone = () => {
    setup.go('phone');
  };
  const FOOT: Record<CallSetupStep, ReactNode> = {
    intro: (
      <>
        {btn(t('calls.setup.intro.phone'), toPhone)}
        {btn(t('calls.setup.intro.start'), setup.check, true)}
      </>
    ),
    phone: (
      <>
        {btn(t('calls.setup.phone.setUp'), () => {
          setup.go('intro');
        })}
        {btn(t('calls.setup.phone.done'), onClose, true)}
      </>
    ),
    install: (
      <>
        {btn(t('calls.setup.install.back'), () => {
          setup.go('intro');
        })}
        <Button kind="primary" disabled={busy || !setup.agreed} onClick={setup.configure}>
          {t('calls.setup.install.continue')}
        </Button>
      </>
    ),
    pair: (
      <>
        {btn(t('calls.setup.pair.cancel'), onClose)}
        {btn(t('calls.setup.pair.check'), setup.check, true)}
      </>
    ),
    test: (
      <>
        {btn(t('calls.setup.test.back'), () => {
          setup.go('pair');
        })}
        {btn(
          t('calls.setup.test.continue'),
          () => {
            setup.go('done');
          },
          true,
        )}
      </>
    ),
    done: btn(t('calls.setup.done.done'), onClose, true),
    off: (
      <>
        {btn(t('calls.setup.off.phone'), toPhone)}
        {btn(t('calls.setup.off.check'), setup.check, true)}
      </>
    ),
    no_bt: btn(t('calls.setup.no_bt.phone'), toPhone, true),
  };
  return (
    <Dialog
      tone={step === 'off' || step === 'no_bt' ? 'warning' : 'accent'}
      icon={ICON[step]}
      title={t(`calls.setup.${step}.title`)}
      onDismiss={onClose}
      foot={FOOT[step]}
    >
      {setup.note !== null && <Banner tone="warning" title={t(`calls.setup.${setup.note}`)} />}
      <p>{t(`calls.setup.${step}.body`)}</p>
      {step === 'install' && (
        <>
          <List>
            <Row
              title={t('calls.setup.install.package')}
              sub={t('calls.setup.install.packageSub')}
              trail={
                setup.installed ? (
                  <Badge tone="success">{t('calls.setup.install.installed')}</Badge>
                ) : (
                  <Button size="sm" icon="box" disabled={busy} onClick={setup.install}>
                    {t('calls.setup.install.install')}
                  </Button>
                )
              }
            />
          </List>
          <h3 className="s-callsetup__h">{t('calls.setup.install.changeTitle')}</h3>
          <p>{t('calls.setup.install.changeBody')}</p>
          <label className="s-callsetup__agree">
            <Check
              value={setup.agreed}
              label={t('calls.setup.install.agree')}
              onChange={setup.agree}
            />
            <span aria-hidden="true">{t('calls.setup.install.agree')}</span>
          </label>
        </>
      )}
    </Dialog>
  );
}

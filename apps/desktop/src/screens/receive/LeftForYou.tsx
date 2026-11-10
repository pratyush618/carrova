import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, GroupHead, List, Note, Row } from '../../ui';

/** Apps to install here, as a checklist the user ticks for themselves (no commands: the engine has none). */
export function LeftForYou({ apps }: { readonly apps: readonly string[] }) {
  const { t } = useTranslation();
  const [done, setDone] = useState<ReadonlySet<string>>(new Set());
  const toggle = (app: string, on: boolean) => {
    setDone((prev) => {
      const next = new Set(prev);
      if (on) next.add(app);
      else next.delete(app);
      return next;
    });
  };
  return (
    <section aria-label={t('receive.done.finish')}>
      <GroupHead
        title={t('receive.done.finish')}
        aside={
          <Note>{t('receive.done.finishCount', { done: done.size, total: apps.length })}</Note>
        }
      />
      <List label={t('receive.done.finish')}>
        {apps.map((app) => {
          const title = t('receive.done.install', { app });
          return (
            <Row
              key={app}
              lead={
                <Check
                  value={done.has(app)}
                  label={t('receive.done.taskDone', { task: title })}
                  onChange={(on) => {
                    toggle(app, on);
                  }}
                />
              }
              title={title}
              sub={t('receive.done.installSub')}
              off={done.has(app)}
            />
          );
        })}
      </List>
    </section>
  );
}

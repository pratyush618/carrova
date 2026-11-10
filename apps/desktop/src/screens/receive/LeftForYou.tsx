import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Check, GroupHead, List, Row } from '../../ui';
import type { ArrivedIntents, Task } from './model';

function useTaskText(task: Task): { title: string; sub: string } {
  const { t } = useTranslation();
  switch (task.kind) {
    case 'reinstall':
      return {
        title: t('receive.done.reinstall', { count: task.count, app: task.app }),
        sub: t('receive.done.reinstallSub'),
      };
    case 'install':
      return {
        title: t('receive.done.install', { app: task.app }),
        sub: t('receive.done.installSub'),
      };
    case 'new-terminal':
      return { title: t('receive.done.terminal'), sub: t('receive.done.terminalSub') };
    case 'sign-in':
      return {
        title: t('receive.done.signIn', { app: task.app }),
        sub: t('receive.done.signInSub'),
      };
  }
}

function TaskRow({
  task,
  onCopy,
}: {
  readonly task: Task;
  readonly onCopy: ArrivedIntents['onCopyCommand'];
}) {
  const { t } = useTranslation();
  const { title, sub } = useTaskText(task);
  const [done, setDone] = useState(false);
  return (
    <Row
      lead={<Check value={done} label={t('receive.done.taskDone')} onChange={setDone} />}
      title={title}
      sub={sub}
      off={done}
      trail={
        task.kind === 'reinstall' && (
          <Button
            size="sm"
            icon="copy"
            onClick={() => {
              onCopy(task.command);
            }}
          >
            {t('receive.done.copyCommand')}
          </Button>
        )
      }
    />
  );
}

/** What's left for the user, as a short checklist they can tick for themselves. */
export function LeftForYou({
  tasks,
  onCopy,
}: {
  readonly tasks: readonly Task[];
  readonly onCopy: ArrivedIntents['onCopyCommand'];
}) {
  const { t } = useTranslation();
  return (
    <div>
      <GroupHead title={t('receive.done.left')} />
      <List label={t('receive.done.left')}>
        {tasks.map((task) => (
          <TaskRow
            key={task.kind === 'new-terminal' ? task.kind : `${task.kind}-${task.app}`}
            task={task}
            onCopy={onCopy}
          />
        ))}
      </List>
    </div>
  );
}

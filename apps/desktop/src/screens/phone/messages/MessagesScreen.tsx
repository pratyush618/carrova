import { useTranslation } from 'react-i18next';
import type { ThreadRow } from '../../../bindings/ThreadRow';
import type { ThreadView } from '../../../bindings/ThreadView';
import type { SmsSend } from '../../../flows/phone/useSmsSends';
import { Banner, Button, Empty, HubLayout } from '../../../ui';
import { NewMessage } from './NewMessage';
import { ThreadList } from './ThreadList';
import { ThreadPane } from './ThreadPane';
import './messages.css';

export interface MessagesModel {
  /** The phone's name. */
  readonly phone: string;
  /** The phone is not linked now: what was loaded stays readable, sending waits. */
  readonly away: boolean;
  /** The last load did not reach the phone. */
  readonly failed: boolean;
  /** Newest first; `null` until the first page answered. */
  readonly threads: readonly ThreadRow[] | null;
  /** Older conversations wait on the phone. */
  readonly moreThreads: boolean;
  readonly selected: ThreadRow | null;
  readonly thread: ThreadView | null;
  /** Texts sent from here for what is open (the conversation, or the new message). */
  readonly sends: readonly SmsSend[];
  /** Writing a new message. */
  readonly composing: boolean;
}

export interface MessagesIntents {
  readonly onSelect: (id: number) => void;
  readonly onCompose: () => void;
  readonly onSend: (to: string, text: string) => void;
  readonly onRetry: (clientId: number) => void;
  /** Load the open conversation's next page of older texts. */
  readonly onOlderMessages: () => void;
  /** Load the next page of older conversations. */
  readonly onMoreThreads: () => void;
}

/** A linked phone's texts: conversations beside the open one, sent through the phone. */
export function MessagesScreen({
  model,
  on,
}: {
  readonly model: MessagesModel;
  readonly on: MessagesIntents;
}) {
  const { t } = useTranslation();
  const { phone, away, threads, selected } = model;
  const problem = model.failed && (
    <Banner tone="warning" title={t('ecosystem.messages.loadFailed', { name: phone })} />
  );
  if (threads === null) return <HubLayout>{problem}</HubLayout>;
  if (threads.length === 0 && !model.composing)
    return (
      <HubLayout variant="center">
        {problem}
        <Empty
          art="msg"
          title={t('ecosystem.messages.emptyTitle', { name: phone })}
          body={t('ecosystem.messages.emptyBody')}
          action={
            <Button icon="pen" onClick={on.onCompose}>
              {t('ecosystem.messages.newMessage')}
            </Button>
          }
        />
      </HubLayout>
    );
  return (
    <HubLayout
      variant="flush"
      tools={
        <Button size="sm" icon="pen" onClick={on.onCompose}>
          {t('ecosystem.messages.newMessage')}
        </Button>
      }
    >
      <div className="s-msgs">
        <ThreadList
          threads={threads}
          selected={model.composing ? null : (selected?.id ?? null)}
          onSelect={on.onSelect}
          {...(model.moreThreads && { onMore: on.onMoreThreads })}
        />
        <div className="s-msgs__pane">
          {problem}
          {model.composing ? (
            <NewMessage
              threads={threads}
              sends={model.sends}
              phone={phone}
              away={away}
              onSelect={on.onSelect}
              onSend={on.onSend}
              onRetry={on.onRetry}
            />
          ) : (
            selected !== null && (
              <ThreadPane
                key={selected.id}
                row={selected}
                thread={model.thread}
                sends={model.sends}
                phone={phone}
                away={away}
                onSend={on.onSend}
                onRetry={on.onRetry}
                onOlder={on.onOlderMessages}
              />
            )
          )}
        </div>
      </div>
    </HubLayout>
  );
}

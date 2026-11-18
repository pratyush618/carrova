import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { NoteView } from '../../../bindings/NoteView';
import { AppGlyph, Button, IconButton, Input, LinkButton } from '../../../ui';
import './notes.css';

/** What the pop-up shows: one notification, or only a count while this computer is locked. */
export type NotePopupModel =
  | { readonly kind: 'note'; readonly phone: string; readonly note: NoteView }
  | { readonly kind: 'locked'; readonly phone: string; readonly count: number };

export interface NotePopupIntents {
  readonly onReply: (text: string) => void;
  /** Mark as read: dismissed on the phone too. */
  readonly onDismiss: () => void;
  /** Open it on the phone. */
  readonly onOpen: () => void;
  readonly onClose: () => void;
}

/**
 * A phone notification in Carrova's own pop-up window (never the OS's,
 * which keeps a history): the app, title and text, an inline reply where
 * the app offers one, and Mark as read / Open on phone.
 */
export function NotePopup({
  model,
  problem = null,
  busy = false,
  on,
}: {
  readonly model: NotePopupModel;
  /** How the last action went wrong, if it did. */
  readonly problem?: 'gone' | 'failed' | null;
  /** An action is on its way to the phone: no other starts. */
  readonly busy?: boolean;
  readonly on: NotePopupIntents;
}) {
  const { t } = useTranslation();
  const name = model.phone;
  return (
    <section className="s-note" aria-labelledby="s-note-title">
      {model.kind === 'locked' ? (
        <>
          <p className="s-note__from">
            <AppGlyph glyph={{ icon: 'bell' }} size="sm" />
            {t('brand.name')}
          </p>
          <h1 id="s-note-title" className="s-note__title">
            {t('ecosystem.notes.locked', { name, count: model.count })}
          </h1>
          <p>{t('ecosystem.notes.lockedBody')}</p>
        </>
      ) : (
        <NoteBody note={model.note} phone={name} busy={busy} on={on} />
      )}
      {problem !== null && (
        <p className="s-note__problem" role="alert">
          {t(`ecosystem.notes.${problem}`, { name })}
        </p>
      )}
      <IconButton icon="close" label={t('ecosystem.notes.close')} onClick={on.onClose} />
    </section>
  );
}

function NoteBody({
  note,
  phone,
  busy,
  on,
}: {
  readonly note: NoteView;
  readonly phone: string;
  readonly busy: boolean;
  readonly on: NotePopupIntents;
}) {
  const { t } = useTranslation();
  const [reply, setReply] = useState('');
  const send = () => {
    if (!busy && reply.trim() !== '') on.onReply(reply);
  };
  return (
    <>
      <p className="s-note__from">
        {note.icon === null ? (
          <AppGlyph glyph={{ icon: 'bell' }} size="sm" />
        ) : (
          <img className="s-note__icon" src={note.icon} alt="" />
        )}
        {t('ecosystem.notes.from', { app: note.app_name, name: phone })}
      </p>
      <h1 id="s-note-title" className="s-note__title">
        {note.title}
      </h1>
      {note.hidden === 'code' ? (
        <p className="s-note__muted">{t('ecosystem.notes.code')}</p>
      ) : (
        note.text !== '' && <p className="s-note__text">{note.text}</p>
      )}
      {note.can_reply && (
        <form
          className="s-note__reply"
          onSubmit={(e) => {
            e.preventDefault();
            send();
          }}
        >
          <Input
            aria-label={t('ecosystem.notes.reply')}
            placeholder={t('ecosystem.notes.replyHint')}
            value={reply}
            readOnly={busy}
            onChange={(e) => {
              setReply(e.target.value);
            }}
          />
          <Button kind="primary" size="sm" disabled={busy || reply.trim() === ''} onClick={send}>
            {t('ecosystem.notes.send')}
          </Button>
        </form>
      )}
      <p className="s-note__acts">
        <LinkButton disabled={busy} onClick={on.onDismiss}>
          {t('ecosystem.notes.dismiss')}
        </LinkButton>
        <LinkButton disabled={busy} onClick={on.onOpen}>
          {t('ecosystem.notes.open')}
        </LinkButton>
      </p>
    </>
  );
}

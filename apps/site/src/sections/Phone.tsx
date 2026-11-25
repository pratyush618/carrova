import type { IconName } from '../../../desktop/src/ui/Icon/icons';
import { Icon } from '../Icon';
import { LiveFrame } from '../LiveFrame';
import { Shot } from '../Shot';

const FEATURES: readonly { icon: IconName; text: string }[] = [
  { icon: 'copy', text: 'Copy on one, paste on the other.' },
  { icon: 'file', text: 'Send files both ways.' },
  { icon: 'bell', text: 'See phone notifications on your computer.' },
  { icon: 'msg', text: 'Read and reply to texts.' },
  { icon: 'call', text: 'Take calls on your laptop.' },
  { icon: 'ring', text: 'Ring your phone when it’s lost.' },
];

export function Phone() {
  return (
    <section className="phone" id="phone" aria-labelledby="phone-title">
      <h2 id="phone-title" className="section-title">
        Your Android phone, too.
      </h2>
      <p className="section-sub">
        Link your phone once and it works with your computer over the same private connection.
        Android 13 or newer, in beta.
      </p>
      <div className="phone-live">
        <LiveFrame
          scene="phone"
          title="Try a linked phone: a text, a call, Ring, and the shared clipboard"
        />
      </div>
      <div className="phone-stage">
        <div className="window window-free phone-back">
          <Shot
            name="messages"
            sizes="(min-width: 1360px) 760px, 70vw"
            alt="Messages on the computer: a text from Sam Rivera, sent through the linked Pixel 8, with a reply box."
          />
        </div>
        <div className="window window-free phone-front">
          <Shot
            name="device"
            sizes="(min-width: 1360px) 760px, 80vw"
            alt="The Pixel 8's page on the computer: Ring, Send clipboard and Send files, with switches for shared clipboard, notifications, messages and calls."
          />
        </div>
        <div className="window window-free phone-note">
          <Shot
            name="note"
            sizes="380px"
            alt="A notification from the phone on the computer: Sam Rivera asks “Are you still coming tonight?”, with a reply box."
          />
        </div>
      </div>
      <ul className="features">
        {FEATURES.map((f) => (
          <li key={f.text}>
            <Icon name={f.icon} />
            {f.text}
          </li>
        ))}
      </ul>
    </section>
  );
}

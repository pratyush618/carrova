import type { IconName } from '../../../desktop/src/ui/Icon/icons';
import { Icon } from '../Icon';

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
      </p>
      <ul className="features">
        {FEATURES.map((f) => (
          <li key={f.text}>
            <Icon name={f.icon} />
            {f.text}
          </li>
        ))}
      </ul>
      <p className="fine">Android 13 or newer. Beta.</p>
    </section>
  );
}

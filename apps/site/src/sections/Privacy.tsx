import type { IconName } from '../../../desktop/src/ui/Icon/icons';
import { Icon } from '../Icon';
import { LiveFrame } from '../LiveFrame';
import { Shot } from '../Shot';

const FACTS: readonly { icon: IconName; lead: string; body: string }[] = [
  {
    icon: 'wifi',
    lead: 'No account. No cloud.',
    body: 'Your data goes straight from one computer to the other over your own network. No server ever holds it.',
  },
  {
    icon: 'lock',
    lead: 'A code only you two see.',
    body: 'Computers pair with a short code shown on both screens. Nobody else on the network can join in.',
  },
  {
    icon: 'shield',
    lead: 'Encrypted end to end.',
    body: 'Everything that travels is encrypted, and so is the file if you move by USB stick instead.',
  },
  {
    icon: 'key',
    lead: 'Keys ask first.',
    body: 'SSH keys, GPG keys and saved tokens stay behind until you say yes to each one.',
  },
];

export function Privacy() {
  return (
    <section className="privacy" id="privacy" aria-labelledby="privacy-title">
      <h2 id="privacy-title" className="section-title">
        Private by design.
        <br />
        <span className="dim">Not by promise.</span>
      </h2>
      <div className="window privacy-live">
        <LiveFrame
          scene="linked"
          title="Two computers linked, with encrypted traffic crossing between them"
          decorative
          playOnView
        />
      </div>
      <div className="window privacy-shot">
        <Shot
          name="linked"
          sizes="(min-width: 1360px) 1200px, calc(100vw - 32px)"
          alt="Linked. Only ezio-thinkpad-new can read this. Both computers proved they know the code without sending it, and everything from here on is encrypted end to end."
        />
      </div>
      <ul className="facts">
        {FACTS.map((f) => (
          <li key={f.lead}>
            <Icon name={f.icon} />
            <p>
              <strong>{f.lead}</strong> {f.body}
            </p>
          </li>
        ))}
      </ul>
      <p className="fine">
        Open source under Apache-2.0, so anyone can check.{' '}
        <a className="more" href="https://github.com/pratyush618/carrova/blob/main/SECURITY.md">
          Security policy
        </a>
      </p>
    </section>
  );
}

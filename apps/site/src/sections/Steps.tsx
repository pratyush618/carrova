import { useEffect, useRef, useState } from 'react';
import { LiveFrame } from '../LiveFrame';
import { Shot, type ShotName } from '../Shot';

const STEPS: readonly { title: string; body: string; shot: ShotName; alt: string }[] = [
  {
    title: 'Pick what moves.',
    body: 'On your old computer, Carrova finds your apps and suggests what to bring. Keys and saved logins start off until you include them.',
    shot: 'send',
    alt: 'Here’s what we found: 5 apps on this computer, with SSH keys, GnuPG keys and Git credentials off until you include them.',
  },
  {
    title: 'Type three words.',
    body: 'Your old computer shows a short code. Type it on the new one and the two pair privately. The code itself never crosses the network.',
    shot: 'code',
    alt: 'Type this on the other computer: the code 7-zebra-lemon, waiting for the other computer.',
  },
  {
    title: 'Arrive.',
    body: 'Your settings land in place. Carrova lists the apps to install, and they open already set up the way you left them.',
    shot: 'done',
    alt: 'Everything arrived: 6 apps in place, with VS Code and tmux left to install.',
  },
];

/** The step whose stage is the live pairing, not a screenshot. */
const PAIR = 1;

/** Text scrolls by while the screenshot stays put and follows the step in view. */
export function Steps() {
  const [active, setActive] = useState(0);
  // Each time the pairing step comes into view, the live code plays again.
  const [pairPlays, setPairPlays] = useState(0);
  const refs = useRef<(HTMLElement | null)[]>([]);

  useEffect(() => {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting) continue;
          const step = Number((e.target as HTMLElement).dataset['step']);
          setActive(step);
          if (step === PAIR) setPairPlays((n) => n + 1);
        }
      },
      { rootMargin: '-45% 0px -45% 0px' },
    );
    for (const el of refs.current) if (el) io.observe(el);
    return () => {
      io.disconnect();
    };
  }, []);

  return (
    <section className="steps" id="how" aria-labelledby="how-title">
      <h2 id="how-title" className="section-title">
        Three steps. One evening, tops.
      </h2>
      <div className="steps-grid">
        <ol className="steps-text">
          {STEPS.map((s, i) => (
            <li
              key={s.title}
              data-step={i}
              ref={(el) => {
                refs.current[i] = el;
              }}
              className={i === active ? 'is-active' : undefined}
            >
              <h3>{s.title}</h3>
              <p>{s.body}</p>
              <div className="window steps-inline">
                <Shot name={s.shot} sizes="calc(100vw - 32px)" alt={s.alt} />
              </div>
            </li>
          ))}
        </ol>
        <div className="steps-stage" aria-hidden="true">
          <div className="window">
            {STEPS.map((s, i) => (
              <div key={s.title} className={i === active ? 'stage-layer is-active' : 'stage-layer'}>
                {i === PAIR ? (
                  <LiveFrame
                    scene="pair"
                    title="Carrova pairing, live"
                    play={pairPlays}
                    decorative
                  />
                ) : (
                  <Shot name={s.shot} sizes="(min-width: 1360px) 760px, 58vw" alt="" />
                )}
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
